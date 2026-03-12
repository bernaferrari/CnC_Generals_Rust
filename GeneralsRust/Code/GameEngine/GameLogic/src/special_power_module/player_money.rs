//! Player Money System for Special Powers
//!
//! Manages player money/resources and cost handling for special power activation.
//! Matches C++ Player class money management.

use crate::common::*;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

/// Player money/resource tracker
#[derive(Debug, Clone)]
pub struct PlayerMoney {
    /// Player ID
    pub player_id: ObjectID,
    /// Current money amount
    pub current_money: Int,
    /// Total money earned (for statistics)
    pub total_earned: Int,
    /// Total money spent (for statistics)
    pub total_spent: Int,
    /// Money income rate per second
    pub income_rate: Real,
}

impl PlayerMoney {
    pub fn new(player_id: ObjectID, starting_money: Int) -> Self {
        Self {
            player_id,
            current_money: starting_money,
            total_earned: starting_money,
            total_spent: 0,
            income_rate: 0.0,
        }
    }

    /// Get current money amount
    pub fn get_money(&self) -> Int {
        self.current_money
    }

    /// Check if player can afford cost
    /// Matches C++ Player::canAfford(int cost)
    pub fn can_afford(&self, cost: Int) -> bool {
        self.current_money >= cost
    }

    /// Add money to player
    /// Matches C++ Player::addMoney(int amount)
    pub fn add_money(&mut self, amount: Int) {
        self.current_money += amount;
        self.total_earned += amount;
        log::debug!(
            "Player {} received ${}, now has ${}",
            self.player_id,
            amount,
            self.current_money
        );
    }

    /// Deduct money from player
    /// Matches C++ Player::spendMoney(int amount)
    /// Returns true if successful, false if insufficient funds
    pub fn spend_money(&mut self, amount: Int) -> bool {
        if !self.can_afford(amount) {
            log::warn!(
                "Player {} cannot afford ${} (has ${})",
                self.player_id,
                amount,
                self.current_money
            );
            return false;
        }

        self.current_money -= amount;
        self.total_spent += amount;
        log::debug!(
            "Player {} spent ${}, now has ${}",
            self.player_id,
            amount,
            self.current_money
        );
        true
    }

    /// Try to deduct money, returns Result
    pub fn try_spend(&mut self, amount: Int) -> Result<(), String> {
        if self.spend_money(amount) {
            Ok(())
        } else {
            Err(format!(
                "Insufficient funds: need ${}, have ${}",
                amount, self.current_money
            ))
        }
    }

    /// Set income rate
    pub fn set_income_rate(&mut self, rate: Real) {
        self.income_rate = rate;
    }

    /// Update money based on income (call every second)
    pub fn update_income(&mut self, delta_time: Real) {
        if self.income_rate > 0.0 {
            let income = (self.income_rate * delta_time) as Int;
            if income > 0 {
                self.add_money(income);
            }
        }
    }

    /// Get total spent
    pub fn get_total_spent(&self) -> Int {
        self.total_spent
    }

    /// Get total earned
    pub fn get_total_earned(&self) -> Int {
        self.total_earned
    }

    /// Reset money to starting amount
    pub fn reset(&mut self, starting_money: Int) {
        self.current_money = starting_money;
        self.total_earned = starting_money;
        self.total_spent = 0;
        self.income_rate = 0.0;
    }
}

/// Money transaction record (for logging/debugging)
#[derive(Debug, Clone)]
pub struct MoneyTransaction {
    pub player_id: ObjectID,
    pub amount: Int,
    pub transaction_type: TransactionType,
    pub description: AsciiString,
    pub frame: UnsignedInt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Income,
    Expense,
    Bounty,
    Hack,
}

/// Global player money manager
static PLAYER_MONEY_MANAGER: OnceLock<Arc<RwLock<PlayerMoneyManager>>> = OnceLock::new();

/// Manager for all player money systems
#[derive(Debug)]
pub struct PlayerMoneyManager {
    players: HashMap<ObjectID, PlayerMoney>,
    transaction_log: Vec<MoneyTransaction>,
    log_enabled: bool,
}

impl PlayerMoneyManager {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            transaction_log: Vec::new(),
            log_enabled: false,
        }
    }

    /// Register a player with starting money
    pub fn register_player(&mut self, player_id: ObjectID, starting_money: Int) {
        self.players
            .insert(player_id, PlayerMoney::new(player_id, starting_money));
    }

    /// Get player money
    pub fn get_player(&self, player_id: ObjectID) -> Option<&PlayerMoney> {
        self.players.get(&player_id)
    }

    /// Get mutable player money
    pub fn get_player_mut(&mut self, player_id: ObjectID) -> Option<&mut PlayerMoney> {
        self.players.get_mut(&player_id)
    }

    /// Get player's current money
    pub fn get_money(&self, player_id: ObjectID) -> Int {
        self.players
            .get(&player_id)
            .map(|p| p.get_money())
            .unwrap_or(0)
    }

    /// Check if player can afford cost
    pub fn can_afford(&self, player_id: ObjectID, cost: Int) -> bool {
        self.players
            .get(&player_id)
            .map(|p| p.can_afford(cost))
            .unwrap_or(false)
    }

    /// Add money to player
    pub fn add_money(&mut self, player_id: ObjectID, amount: Int, current_frame: UnsignedInt) {
        if let Some(player) = self.players.get_mut(&player_id) {
            player.add_money(amount);

            if self.log_enabled {
                self.transaction_log.push(MoneyTransaction {
                    player_id,
                    amount,
                    transaction_type: TransactionType::Income,
                    description: "Income".into(),
                    frame: current_frame,
                });
            }
        }
    }

    /// Deduct money from player
    pub fn spend_money(
        &mut self,
        player_id: ObjectID,
        amount: Int,
        current_frame: UnsignedInt,
    ) -> bool {
        if let Some(player) = self.players.get_mut(&player_id) {
            let success = player.spend_money(amount);

            if success && self.log_enabled {
                self.transaction_log.push(MoneyTransaction {
                    player_id,
                    amount,
                    transaction_type: TransactionType::Expense,
                    description: "Expense".into(),
                    frame: current_frame,
                });
            }

            success
        } else {
            false
        }
    }

    /// Transfer money between players (for Cash Hack power)
    pub fn transfer_money(
        &mut self,
        from_player: ObjectID,
        to_player: ObjectID,
        amount: Int,
        current_frame: UnsignedInt,
    ) -> bool {
        // Check if source has enough money
        let can_transfer = self
            .players
            .get(&from_player)
            .map(|p| p.can_afford(amount))
            .unwrap_or(false);

        if !can_transfer {
            return false;
        }

        // Deduct from source
        if let Some(source) = self.players.get_mut(&from_player) {
            source.current_money -= amount;
            source.total_spent += amount;
        }

        // Add to target
        if let Some(target) = self.players.get_mut(&to_player) {
            target.add_money(amount);
        }

        if self.log_enabled {
            self.transaction_log.push(MoneyTransaction {
                player_id: to_player,
                amount,
                transaction_type: TransactionType::Hack,
                description: format!("Stolen from player {}", from_player).into(),
                frame: current_frame,
            });
        }

        log::info!(
            "Transferred ${} from player {} to player {}",
            amount,
            from_player,
            to_player
        );

        true
    }

    /// Update all players' income
    pub fn update_all(&mut self, delta_time: Real) {
        for player in self.players.values_mut() {
            player.update_income(delta_time);
        }
    }

    /// Enable transaction logging
    pub fn enable_logging(&mut self) {
        self.log_enabled = true;
    }

    /// Get transaction log
    pub fn get_transaction_log(&self) -> &[MoneyTransaction] {
        &self.transaction_log
    }

    /// Clear transaction log
    pub fn clear_log(&mut self) {
        self.transaction_log.clear();
    }
}

impl Default for PlayerMoneyManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize global player money manager
pub fn initialize_player_money() {
    let _ = PLAYER_MONEY_MANAGER.get_or_init(|| Arc::new(RwLock::new(PlayerMoneyManager::new())));
}

/// Get global player money manager
pub fn get_player_money_manager() -> Option<Arc<RwLock<PlayerMoneyManager>>> {
    PLAYER_MONEY_MANAGER.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_money_basic() {
        let mut player = PlayerMoney::new(1, 10000);

        assert_eq!(player.get_money(), 10000);
        assert!(player.can_afford(5000));
        assert!(!player.can_afford(15000));
    }

    #[test]
    fn test_spend_money() {
        let mut player = PlayerMoney::new(1, 10000);

        assert!(player.spend_money(3000));
        assert_eq!(player.get_money(), 7000);
        assert_eq!(player.get_total_spent(), 3000);

        assert!(!player.spend_money(10000)); // Can't afford
        assert_eq!(player.get_money(), 7000); // Money unchanged
    }

    #[test]
    fn test_add_money() {
        let mut player = PlayerMoney::new(1, 10000);

        player.add_money(5000);
        assert_eq!(player.get_money(), 15000);
        assert_eq!(player.get_total_earned(), 15000); // Includes starting money
    }

    #[test]
    fn test_income_rate() {
        let mut player = PlayerMoney::new(1, 10000);
        player.set_income_rate(100.0); // $100 per second

        player.update_income(1.0); // 1 second
        assert_eq!(player.get_money(), 10100);

        player.update_income(5.0); // 5 seconds
        assert_eq!(player.get_money(), 10600);
    }

    #[test]
    fn test_player_money_manager() {
        let mut manager = PlayerMoneyManager::new();

        manager.register_player(1, 10000);
        manager.register_player(2, 5000);

        assert_eq!(manager.get_money(1), 10000);
        assert_eq!(manager.get_money(2), 5000);

        assert!(manager.can_afford(1, 5000));
        assert!(!manager.can_afford(2, 10000));
    }

    #[test]
    fn test_money_transfer() {
        let mut manager = PlayerMoneyManager::new();

        manager.register_player(1, 10000);
        manager.register_player(2, 5000);

        // Transfer $3000 from player 1 to player 2
        assert!(manager.transfer_money(1, 2, 3000, 0));

        assert_eq!(manager.get_money(1), 7000);
        assert_eq!(manager.get_money(2), 8000);

        // Can't transfer more than available
        assert!(!manager.transfer_money(2, 1, 10000, 0));
        assert_eq!(manager.get_money(2), 8000); // Unchanged
    }

    #[test]
    fn test_transaction_logging() {
        let mut manager = PlayerMoneyManager::new();
        manager.enable_logging();

        manager.register_player(1, 10000);

        manager.spend_money(1, 1000, 0);
        manager.add_money(1, 500, 1);

        let log = manager.get_transaction_log();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].transaction_type, TransactionType::Expense);
        assert_eq!(log[1].transaction_type, TransactionType::Income);
    }
}
