//! Player List System
//!
//! Manages the list of all players in the game.
//! Ported from GeneralsMD/Code/GameEngine/Source/Common/RTS/PlayerList.cpp
//! C++ Header: GeneralsMD/Code/GameEngine/Include/Common/PlayerList.h

use crate::common::rts::{Player, Relationship};
use crate::common::system::{Snapshotable, Xfer, XferMode, XferVersion};

/// Maximum number of players
/// C++ Reference: MAX_PLAYER_COUNT defined in GameCommon.h
pub const MAX_PLAYER_COUNT: usize = 8;

/// Allow player relationship flags
/// C++ Reference: AllowPlayerRelationship enum in PlayerList.h
pub const ALLOW_SAME_PLAYER: u32 = 0x01;
pub const ALLOW_ALLIES: u32 = 0x02;
pub const ALLOW_ENEMIES: u32 = 0x04;
pub const ALLOW_NEUTRAL: u32 = 0x08;

/// Player list manager
///
/// C++ Reference: PlayerList class in PlayerList.h
/// This is a singleton class that maintains the list of Players.
#[derive(Debug)]
pub struct PlayerList {
    /// Array of all player slots
    /// C++: m_players[MAX_PLAYER_COUNT]
    players: [Player; MAX_PLAYER_COUNT],
    /// Number of active players (including neutral)
    /// C++: m_playerCount
    player_count: usize,
    /// Index of the local (human) player
    /// C++: m_local
    local_player_index: Option<usize>,
}

impl PlayerList {
    pub fn new() -> Self {
        let mut list = Self {
            players: std::array::from_fn(|i| Player::new(i as i32)),
            player_count: 1,
            local_player_index: None,
        };
        list.init();
        list
    }

    /// Initialize the player list.
    /// C++ Reference: PlayerList::init() lines 195-210
    ///
    /// Sets player count to 1 (neutral player only), initializes all players,
    /// and sets player 0 as the local player (neutral player).
    pub fn init(&mut self) {
        self.player_count = 1;
        
        // Initialize player 0 (neutral) with no name
        self.players[0].init(None);

        // Initialize remaining players with no name
        for i in 1..MAX_PLAYER_COUNT {
            self.players[i].init(None);
        }

        // Call set_local_player so that becoming_local_player() gets called appropriately
        // In C++ this calls becomingLocalPlayer() on the player
        self.set_local_player(0);
    }

    /// Start a new game, creating players from the provided side information.
    /// C++ Reference: PlayerList::newGame() lines 90-180
    ///
    /// This method creates all players from the sides data, sets up relationships
    /// (allies/enemies), and configures the local player.
    ///
    /// # Arguments
    /// * `sides` - Slice of SideInfo containing player configuration
    /// * `get_player_name` - Function to get player name from SideInfo
    /// * `is_human` - Function to check if player is human
    /// * `get_allies` - Function to get allies list (space-separated names)
    /// * `get_enemies` - Function to get enemies list (space-separated names)
    /// * `is_multiplayer_local` - Optional function to check if player is multiplayer local
    /// * `is_network_active` - Whether network is active (affects local player selection)
    pub fn new_game<S, F1, F2, F3, F4, F5>(
        &mut self,
        sides: &[S],
        get_player_name: F1,
        is_human: F2,
        get_allies: F3,
        get_enemies: F4,
        is_multiplayer_local: Option<F5>,
        is_network_active: bool,
    ) where
        F1: Fn(&S) -> Option<String>,
        F2: Fn(&S) -> bool,
        F3: Fn(&S) -> String,
        F4: Fn(&S) -> String,
        F5: Fn(&S) -> bool,
    {
        // First, re-init ourselves (clears team factory, etc.)
        self.init();

        // Create players from sides
        let mut set_local = false;
        
        for side in sides {
            let name = match get_player_name(side) {
                Some(n) if !n.is_empty() => n,
                _ => continue, // Skip neutral (empty name)
            };

            let player_idx = self.player_count;
            if player_idx >= MAX_PLAYER_COUNT {
                break;
            }

            // Initialize player from side info
            self.players[player_idx].init(Some(name.clone()));
            self.player_count += 1;

            // Check for multiplayer local override
            if let Some(ref check_mp_local) = is_multiplayer_local {
                if check_mp_local(side) {
                    self.set_local_player(player_idx);
                    set_local = true;
                }
            }

            // If not network and player is human, set as local
            if !set_local && !is_network_active && is_human(side) {
                self.set_local_player(player_idx);
                set_local = true;
            }
        }

        // If no local player was set, pick first non-neutral player
        if !set_local {
            for i in 1..self.player_count {
                if i != 0 {
                    // Not neutral player
                    self.set_local_player(i);
                    set_local = true;
                    break;
                }
            }
        }

        // Set up relationships
        self.setup_relationships(sides, get_player_name, get_allies, get_enemies);
    }

    /// Set up player relationships (allies/enemies).
    /// C++ Reference: PlayerList::newGame() lines 155-180
    fn setup_relationships<S, F1, F3, F4>(
        &mut self,
        sides: &[S],
        get_player_name: F1,
        get_allies: F3,
        get_enemies: F4,
    ) where
        F1: Fn(&S) -> Option<String>,
        F3: Fn(&S) -> String,
        F4: Fn(&S) -> String,
    {
        for side in sides {
            let name = match get_player_name(side) {
                Some(n) if !n.is_empty() => n,
                _ => continue,
            };

            let player_idx = match self.find_player_by_name(&name) {
                Some(idx) => idx,
                None => continue,
            };

            // Set up enemies
            let enemies = get_enemies(side);
            for enemy_name in enemies.split_whitespace() {
                if let Some(enemy_idx) = self.find_player_by_name(enemy_name) {
                    self.players[player_idx].set_player_relationship(enemy_idx as i32, Relationship::Enemies);
                }
            }

            // Set up allies
            let allies = get_allies(side);
            for ally_name in allies.split_whitespace() {
                if let Some(ally_idx) = self.find_player_by_name(ally_name) {
                    self.players[player_idx].set_player_relationship(ally_idx as i32, Relationship::Allies);
                }
            }

            // Make sure self is allied with self
            self.players[player_idx].set_player_relationship(player_idx as i32, Relationship::Allies);

            // Make sure neutral player relationship is neutral (if not the neutral player)
            if player_idx != 0 {
                self.players[player_idx].set_player_relationship(0, Relationship::Neutral);
            }
        }
    }

    /// Find a player by name.
    /// Returns the player index if found.
    /// C++ Reference: PlayerList::findPlayerWithNameKey()
    pub fn find_player_by_name(&self, name: &str) -> Option<usize> {
        for i in 0..self.player_count {
            if self.players[i].get_player_name() == name {
                return Some(i);
            }
        }
        None
    }

    /// Set the local player by index.
    /// C++ Reference: PlayerList::setLocalPlayer() lines 310-350
    pub fn set_local_player(&mut self, index: usize) {
        if index >= MAX_PLAYER_COUNT {
            return;
        }

        // Can't set local player to null - if you try, you get neutral
        let new_index = index;

        if self.local_player_index != Some(new_index) {
            // Old local player stops being local
            // (In full implementation, this would call becomingLocalPlayer(false))
            
            // Set new local player
            self.local_player_index = Some(new_index);
            
            // (In full implementation, this would call becomingLocalPlayer(true))
        }
    }

    /// Reset the player list (clear teams and reinit).
    /// C++ Reference: PlayerList::reset() lines 80-88
    pub fn reset(&mut self) {
        // In C++: TheTeamFactory->clear()
        // For now, just re-init
        self.init();
    }

    /// Update all players (called each frame).
    /// C++ Reference: PlayerList::update() lines 215-225
    pub fn update(&mut self) {
        for i in 0..MAX_PLAYER_COUNT {
            self.players[i].update();
        }
    }

    /// Handle new map loaded.
    /// C++ Reference: PlayerList::newMap() lines 230-240
    pub fn new_map(&mut self) {
        for i in 0..MAX_PLAYER_COUNT {
            // In full implementation, this would call player.newMap()
            // For now, just re-init the player
            self.players[i].init(None);
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

    pub fn get_local_player_mut(&mut self) -> Option<&mut Player> {
        self.local_player_index.and_then(|i| self.get_nth_player_mut(i))
    }

    pub fn get_local_player_index(&self) -> Option<usize> {
        self.local_player_index
    }

    pub fn get_neutral_player(&self) -> &Player {
        &self.players[0] // Player 0 is always neutral
    }

    pub fn get_neutral_player_mut(&mut self) -> &mut Player {
        &mut self.players[0]
    }

    /// Find a player by name key.
    /// C++ Reference: PlayerList::findPlayerWithNameKey() lines 72-80
    ///
    /// In C++, this uses NameKeyType (a hash of the player name) for fast lookup.
    /// In Rust, we use string comparison directly since we don't have a global name key generator.
    ///
    /// # Arguments
    /// * `name_key` - The name key to search for (currently unused, searches by name instead)
    /// * `name` - The player name to search for
    ///
    /// # Returns
    /// Reference to the player if found, None otherwise
    pub fn find_player_with_name_key(&self, _name_key: u32, name: &str) -> Option<&Player> {
        // C++ uses NameKeyType for lookup, but we search by name string
        for i in 0..self.player_count {
            if self.players[i].get_player_name() == name {
                return Some(&self.players[i]);
            }
        }
        None
    }

    /// Find a player by name key (mutable version).
    /// C++ Reference: PlayerList::findPlayerWithNameKey() lines 72-80
    pub fn find_player_with_name_key_mut(&mut self, _name_key: u32, name: &str) -> Option<&mut Player> {
        for i in 0..self.player_count {
            if self.players[i].get_player_name() == name {
                return Some(&mut self.players[i]);
            }
        }
        None
    }

    /// Get player from a player mask.
    /// C++ Reference: PlayerList::getPlayerFromMask() lines 355-370
    ///
    /// Each player has a unique bitmask (1 << playerIndex). This method finds
    /// the player whose mask matches the given mask.
    ///
    /// # Arguments
    /// * `mask` - The player mask to search for
    ///
    /// # Returns
    /// Reference to the player if found, None otherwise
    pub fn get_player_from_mask(&self, mask: u32) -> Option<&Player> {
        for i in 0..MAX_PLAYER_COUNT {
            let player = &self.players[i];
            if player.get_player_mask() == mask {
                return Some(player);
            }
        }
        None
    }

    /// Get each player from a mask (iterative).
    /// C++ Reference: PlayerList::getEachPlayerFromMask() lines 375-395
    ///
    /// This method finds players whose mask bits are set in the given mask,
    /// and removes their bits from the mask as they are returned.
    /// This allows iterating through all players represented by a combined mask.
    ///
    /// # Arguments
    /// * `mask_to_adjust` - The combined mask, will be modified to remove found player's bit
    ///
    /// # Returns
    /// Reference to the first player found with a matching bit, or None if mask is empty
    pub fn get_each_player_from_mask(&mut self, mask_to_adjust: &mut u32) -> Option<&Player> {
        for i in 0..MAX_PLAYER_COUNT {
            let player = &self.players[i];
            let player_mask = player.get_player_mask();

            // Check if this player's bit is set in the mask
            if (*mask_to_adjust & player_mask) != 0 {
                // Remove this player's bit from the mask
                *mask_to_adjust &= !player_mask;
                return Some(player);
            }
        }
        // No more players found, clear the mask
        *mask_to_adjust = 0;
        None
    }

    /// Get all players matching a relationship filter.
    /// C++ Reference: PlayerList::getPlayersWithRelationship() lines 400-432
    ///
    /// Returns a bitmask of all players that the source player considers
    /// to have one of the specified relationships.
    ///
    /// # Arguments
    /// * `src_player_index` - Index of the source player
    /// * `allowed_relationships` - Bitwise OR of ALLOW_* flags
    ///
    /// # Returns
    /// Bitmask of matching players
    pub fn get_players_with_relationship(
        &self,
        src_player_index: usize,
        allowed_relationships: u32,
    ) -> u32 {
        let mut result: u32 = 0;

        if allowed_relationships == 0 {
            return result;
        }

        let src_player = match self.get_nth_player(src_player_index) {
            Some(p) => p,
            None => return result,
        };

        // Check for ALLOW_SAME_PLAYER
        if (allowed_relationships & ALLOW_SAME_PLAYER) != 0 {
            result |= src_player.get_player_mask();
        }

        // Check all other players
        for i in 0..self.player_count {
            let player = match self.get_nth_player(i) {
                Some(p) => p,
                None => continue,
            };

            // Skip the source player (already handled above)
            if i == src_player_index {
                continue;
            }

            // Get relationship from source player's perspective
            let relationship = src_player.get_relationship(i as i32);

            match relationship {
                Relationship::Enemies => {
                    if (allowed_relationships & ALLOW_ENEMIES) != 0 {
                        result |= player.get_player_mask();
                    }
                }
                Relationship::Allies => {
                    if (allowed_relationships & ALLOW_ALLIES) != 0 {
                        result |= player.get_player_mask();
                    }
                }
                Relationship::Neutral => {
                    if (allowed_relationships & ALLOW_NEUTRAL) != 0 {
                        result |= player.get_player_mask();
                    }
                }
            }
        }

        result
    }

    /// Validate and return a team for a given owner name.
    /// C++ Reference: PlayerList::validateTeam() lines 260-275
    ///
    /// The owner could be a player or team name. First checks team names,
    /// then falls back to the neutral player's default team if not found.
    ///
    /// # Note
    /// In the current implementation, this returns the default team of the
    /// neutral player since TeamFactory integration is not complete.
    /// Full implementation would use TheTeamFactory->findTeam().
    pub fn validate_team(&self, _owner: &str) -> Option<usize> {
        // In full implementation:
        // 1. Check if owner is a team name: TheTeamFactory->findTeam(owner)
        // 2. If not found, check if owner is a player name
        // 3. Return neutral player's default team if nothing found

        // For now, return the neutral player index
        Some(0)
    }

    /// Update team states for all players.
    /// C++ Reference: PlayerList::updateTeamStates() lines 245-255
    ///
    /// Clears team flags (entered/exited) for all players.
    pub fn update_team_states(&mut self) {
        for i in 0..MAX_PLAYER_COUNT {
            // In full implementation, this would call player.updateTeamStates()
            // For now, it's a no-op since we don't have full team integration
            let _ = &mut self.players[i];
        }
    }

    /// Notify all players that a team is about to be deleted.
    /// C++ Reference: PlayerList::teamAboutToBeDeleted() lines 285-295
    ///
    /// # Arguments
    /// * `_team_id` - ID of the team being deleted
    pub fn team_about_to_be_deleted(&mut self, _team_id: usize) {
        for i in 0..MAX_PLAYER_COUNT {
            // In full implementation, this would call player.removeTeamRelationship(team)
            let _ = &mut self.players[i];
        }
    }

    /// Set the local player by reference.
    /// C++ Reference: PlayerList::setLocalPlayer() lines 310-350
    ///
    /// # Arguments
    /// * `player` - Reference to the player to set as local
    pub fn set_local_player_by_ref(&mut self, player: &Player) {
        let index = player.get_player_index() as usize;
        self.set_local_player(index);
    }
}

impl Default for PlayerList {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================
// Snapshotable Implementation (save/load and CRC)
// C++ Reference: PlayerList.cpp lines 407-465
// =========================================================

impl Snapshotable for PlayerList {
    /// CRC computation for network synchronization.
    /// C++ Reference: PlayerList::crc() lines 407-415
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Xfer the player count
        let mut player_count = self.player_count as i32;
        xfer.xfer_int(&mut player_count)
            .map_err(|e| format!("player_count crc failed: {}", e))?;

        // Xfer each player's snapshot
        for i in 0..self.player_count {
            Snapshotable::crc(&self.players[i], xfer)
                .map_err(|e| format!("player {} crc failed: {}", i, e))?;
        }

        Ok(())
    }

    /// Save/load player list state.
    /// C++ Reference: PlayerList::xfer() lines 420-465
    /// Version History:
    ///   1: Initial version
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;

        // Xfer version
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("xfer_version failed: {}", e))?;

        // Xfer the player count
        let mut player_count = self.player_count as i32;
        xfer.xfer_int(&mut player_count)
            .map_err(|e| format!("player_count xfer failed: {}", e))?;

        // Sanity check: the player count read from file should match our player count
        // that was setup from the bare bones map load since that data can't change during runtime
        match xfer.get_xfer_mode() {
            XferMode::Load => {
                if player_count as usize != self.player_count {
                    return Err(format!(
                        "Invalid player count '{}', should be '{}'",
                        player_count, self.player_count
                    ));
                }
            }
            XferMode::Save | XferMode::Crc => {
                // For save/crc, player_count should match
            }
            _ => {}
        }

        // Xfer each of the player data
        for i in 0..player_count as usize {
            if i < MAX_PLAYER_COUNT {
                self.players[i]
                    .xfer(xfer)
                    .map_err(|e| format!("player {} xfer failed: {}", i, e))?;
            }
        }

        Ok(())
    }

    /// Load post process.
    /// C++ Reference: PlayerList::loadPostProcess() lines 470-472
    fn load_post_process(&mut self) -> Result<(), String> {
        // C++ implementation is empty
        Ok(())
    }
}

// =========================================================
// Tests
// =========================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_list_creation() {
        let list = PlayerList::new();
        assert_eq!(list.get_player_count(), 1); // Just neutral player
        assert!(list.get_local_player().is_some());
    }

    #[test]
    fn test_neutral_player() {
        let list = PlayerList::new();
        let neutral = list.get_neutral_player();
        assert_eq!(neutral.get_player_index(), 0);
    }

    #[test]
    fn test_find_player_by_name() {
        let mut list = PlayerList::new();

        // Neutral player has empty name initially
        assert!(list.find_player_by_name("").is_some());
        assert!(list.find_player_by_name("NonExistent").is_none());
    }

    #[test]
    fn test_set_local_player() {
        let mut list = PlayerList::new();

        list.set_local_player(0);
        assert_eq!(list.get_local_player_index(), Some(0));

        // Setting invalid index should not change local player
        list.set_local_player(100);
        assert_eq!(list.get_local_player_index(), Some(0));
    }

    #[test]
    fn test_get_player_from_mask() {
        let list = PlayerList::new();

        // Player 0 has mask 1 << 0 = 1
        let player = list.get_player_from_mask(1);
        assert!(player.is_some());
        assert_eq!(player.unwrap().get_player_index(), 0);

        // No player has mask 0x100
        assert!(list.get_player_from_mask(0x100).is_none());
    }

    #[test]
    fn test_get_each_player_from_mask() {
        let mut list = PlayerList::new();

        // Test with single player (neutral)
        let mut mask = 1u32; // Player 0's mask
        let player = list.get_each_player_from_mask(&mut mask);
        assert!(player.is_some());
        assert_eq!(player.unwrap().get_player_index(), 0);
        assert_eq!(mask, 0); // Mask should be cleared

        // Test with empty mask
        mask = 0;
        let player = list.get_each_player_from_mask(&mut mask);
        assert!(player.is_none());
    }

    #[test]
    fn test_get_players_with_relationship() {
        let list = PlayerList::new();

        // Player 0 (neutral) should match ALLOW_SAME_PLAYER
        let mask = list.get_players_with_relationship(0, ALLOW_SAME_PLAYER);
        assert_eq!(mask, 1); // Player 0's mask

        // No relationships set yet, so no allies/enemies
        let mask = list.get_players_with_relationship(0, ALLOW_ALLIES);
        assert_eq!(mask, 0);

        let mask = list.get_players_with_relationship(0, ALLOW_ENEMIES);
        assert_eq!(mask, 0);
    }

    #[test]
    fn test_reset() {
        let mut list = PlayerList::new();
        list.reset();
        assert_eq!(list.get_player_count(), 1);
    }

    #[test]
    fn test_update() {
        let mut list = PlayerList::new();
        // Should not panic
        list.update();
    }

    #[test]
    fn test_new_map() {
        let mut list = PlayerList::new();
        // Should not panic
        list.new_map();
    }
}
