//! Player List System - Placeholder implementation
//!
//! Manages the list of all players in the game.

use crate::common::rts::Player;

/// Maximum number of players
pub const MAX_PLAYER_COUNT: usize = 8;

/// Player list manager
#[derive(Debug)]
pub struct PlayerList {
    players: [Player; MAX_PLAYER_COUNT],
    player_count: usize,
    local_player_index: Option<usize>,
}

impl PlayerList {
    pub fn new() -> Self {
        Self {
            players: [(); MAX_PLAYER_COUNT].map(|_| Player::default()),
            player_count: 1, // Always have at least neutral player
            local_player_index: None,
        }
    }

    pub fn get_nth_player(&self, index: usize) -> Option<&Player> {
        if index < MAX_PLAYER_COUNT {
            Some(&self.players[index])
        } else {
            None
        }
    }

    pub fn get_nth_player_mut(&mut self, index: usize) -> Option<&mut Player> {
        if index < MAX_PLAYER_COUNT {
            Some(&mut self.players[index])
        } else {
            None
        }
    }

    pub fn get_player_count(&self) -> usize {
        self.player_count
    }

    pub fn get_local_player(&self) -> Option<&Player> {
        self.local_player_index.and_then(|i| self.get_nth_player(i))
    }

    pub fn get_neutral_player(&self) -> &Player {
        &self.players[0] // Player 0 is always neutral
    }
}

impl Default for PlayerList {
    fn default() -> Self {
        Self::new()
    }
}
