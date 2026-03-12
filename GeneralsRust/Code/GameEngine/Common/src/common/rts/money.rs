//! Money management system for RTS gameplay
//!
//! This module handles the player's money/resources (Tiberium, Gems, Magic Resource Boxes, whatever).
//! This is currently a Very Simple Class but is encapsulated in anticipation of future expansion.
//!
//! # C++ Reference
//! - `/GeneralsMD/Code/GameEngine/Source/Common/RTS/Money.cpp`
//! - `/GeneralsMD/Code/GameEngine/Include/Common/Money.h`

/// Money management system
///
/// Tracks how much "money" (resources) a player has and provides
/// methods for depositing and withdrawing funds with sound effects.
///
/// # C++ Reference
/// C++ class Money (Money.h lines 28-71)
pub struct Money {
    /// Amount of money/resources
    /// C++ equivalent: m_money (Money.h line 69)
    money: u32,

    /// Player index for audio events
    /// C++ equivalent: m_playerIndex (Money.h line 70)
    player_index: i32,

    /// Total income received (for tracking)
    total_income: u64,

    /// Total expenditure (for tracking)
    total_spent: u64,

    /// Number of deposits made
    deposit_count: u32,

    /// Number of withdrawals made
    withdrawal_count: u32,

    /// Last deposit timestamp (for rate calculation)
    last_deposit_time: std::time::Instant,

    /// Last withdrawal timestamp (for rate calculation)
    last_withdrawal_time: std::time::Instant,
}

impl Money {
    /// Create a new Money instance
    ///
    /// C++ equivalent: Money::Money() constructor (Money.h lines 33-35)
    pub fn new() -> Self {
        let now = std::time::Instant::now();
        Self {
            money: 0,
            player_index: 0,
            total_income: 0,
            total_spent: 0,
            deposit_count: 0,
            withdrawal_count: 0,
            last_deposit_time: now,
            last_withdrawal_time: now,
        }
    }

    /// Create a new Money instance with initial amount
    pub fn new_with_amount(initial_amount: u32) -> Self {
        let mut money = Self::new();
        money.money = initial_amount;
        money
    }

    /// Initialize/reset money to zero
    ///
    /// C++ equivalent: Money::init() (Money.h lines 37-40)
    pub fn init(&mut self) {
        self.money = 0;
        let now = std::time::Instant::now();
        self.total_income = 0;
        self.total_spent = 0;
        self.deposit_count = 0;
        self.withdrawal_count = 0;
        self.last_deposit_time = now;
        self.last_withdrawal_time = now;
    }

    /// Get the current amount of money
    ///
    /// C++ equivalent: Money::countMoney() (Money.h lines 42-45)
    pub fn count_money(&self) -> u32 {
        self.money
    }

    /// Withdraw money from the account
    ///
    /// Returns the actual amount withdrawn, which may be less than requested
    /// (sorry, can't go into debt...)
    ///
    /// # Arguments
    /// * `amount_to_withdraw` - How much to withdraw
    /// * `play_sound` - Whether to play withdrawal sound effect
    ///
    /// # Returns
    /// The actual amount withdrawn, which may be less than requested
    ///
    /// # C++ Reference
    /// Money::withdraw() (Money.cpp lines 23-42)
    pub fn withdraw(&mut self, amount_to_withdraw: u32, play_sound: bool) -> u32 {
        // C++ Money.cpp line 25-26: Limit withdrawal to available amount
        let actual_withdrawal = if amount_to_withdraw > self.money {
            self.money
        } else {
            amount_to_withdraw
        };

        // C++ Money.cpp line 28-29: Early return if nothing to withdraw
        if actual_withdrawal == 0 {
            return actual_withdrawal;
        }

        // C++ Money.cpp lines 31-37: Audio sound effect
        // @todo: Do we do this frequently enough that it is a performance hit?
        if play_sound {
            // When audio system is available:
            // AudioEventRTS event = TheAudio->getMiscAudio()->m_moneyWithdrawSound;
            // event.setPlayerIndex(self.player_index);
            // TheAudio->addAudioEvent(&event);
            #[cfg(feature = "logging")]
            log::trace!(
                "Money withdrawal sound for player {}: ${}",
                self.player_index,
                actual_withdrawal
            );
        }

        // C++ Money.cpp line 39: Deduct the money
        self.money -= actual_withdrawal;

        // Track statistics
        self.total_spent += actual_withdrawal as u64;
        self.withdrawal_count += 1;
        self.last_withdrawal_time = std::time::Instant::now();

        // C++ Money.cpp line 41: Return actual amount
        actual_withdrawal
    }

    /// Deposit money into the account
    ///
    /// # Arguments
    /// * `amount_to_deposit` - How much to deposit
    /// * `play_sound` - Whether to play deposit sound effect
    ///
    /// # C++ Reference
    /// Money::deposit() (Money.cpp lines 45-68)
    pub fn deposit(&mut self, amount_to_deposit: u32, play_sound: bool) {
        // C++ Money.cpp line 47-48: Early return if nothing to deposit
        if amount_to_deposit == 0 {
            return;
        }

        // C++ Money.cpp lines 50-56: Audio sound effect
        // @todo: Do we do this frequently enough that it is a performance hit?
        if play_sound {
            // When audio system is available:
            // AudioEventRTS event = TheAudio->getMiscAudio()->m_moneyDepositSound;
            // event.setPlayerIndex(self.player_index);
            // TheAudio->addAudioEvent(&event);
            #[cfg(feature = "logging")]
            log::trace!(
                "Money deposit sound for player {}: ${}",
                self.player_index,
                amount_to_deposit
            );
        }

        // C++ Money.cpp line 58: Add the money
        self.money += amount_to_deposit;

        // Track statistics
        self.total_income += amount_to_deposit as u64;
        self.deposit_count += 1;
        self.last_deposit_time = std::time::Instant::now();

        // C++ Money.cpp lines 60-67: Record income for academy stats
        if amount_to_deposit > 0 {
            // When player system is available:
            // Player *player = ThePlayerList->getNthPlayer(self.player_index);
            // if player {
            //     player.get_academy_stats().record_income();
            // }
            #[cfg(feature = "logging")]
            log::trace!(
                "Academy stats: record_income() for player {}",
                self.player_index
            );
        }
    }

    /// Set the player index for audio events
    pub fn set_player_index(&mut self, index: i32) {
        self.player_index = index;
    }

    /// Get the player index
    pub fn get_player_index(&self) -> i32 {
        self.player_index
    }

    /// Check if we have at least the specified amount
    pub fn has_at_least(&self, amount: u32) -> bool {
        self.money >= amount
    }

    /// Check if we can afford a purchase
    pub fn can_afford(&self, cost: u32) -> bool {
        self.money >= cost
    }

    /// Try to purchase something (withdraw if possible)
    ///
    /// # Returns
    /// `true` if purchase was successful, `false` if insufficient funds
    pub fn try_purchase(&mut self, cost: u32, play_sound: bool) -> bool {
        if self.can_afford(cost) {
            self.withdraw(cost, play_sound);
            true
        } else {
            false
        }
    }

    /// Force set the money amount (for cheats, loading save games, etc.)
    pub fn set_money(&mut self, amount: u32) {
        self.money = amount;
    }

    /// Add money without sound effects (for internal use)
    pub fn add_money_silent(&mut self, amount: u32) {
        self.money += amount;
    }

    /// Subtract money without sound effects (for internal use)
    /// Returns the actual amount subtracted
    pub fn subtract_money_silent(&mut self, amount: u32) -> u32 {
        let actual_subtraction = if amount > self.money {
            self.money
        } else {
            amount
        };
        self.money -= actual_subtraction;
        actual_subtraction
    }

    /// Compare money amounts (excluding player index)
    ///
    /// C++ equivalent: Money::amountEqual() (Money.h lines 56-59)
    pub fn amount_equal(&self, other: &Money) -> bool {
        self.money == other.money
    }

    /// Get money as a percentage of a target amount
    pub fn get_percentage_of(&self, target: u32) -> f32 {
        if target == 0 {
            0.0
        } else {
            (self.money as f32 / target as f32) * 100.0
        }
    }

    // ========== Income Tracking and Rate Calculations ==========

    /// Get total income received over lifetime
    pub fn get_total_income(&self) -> u64 {
        self.total_income
    }

    /// Get total amount spent over lifetime
    pub fn get_total_spent(&self) -> u64 {
        self.total_spent
    }

    /// Get net income (income - spent)
    pub fn get_net_income(&self) -> i64 {
        self.total_income as i64 - self.total_spent as i64
    }

    /// Get number of deposits made
    pub fn get_deposit_count(&self) -> u32 {
        self.deposit_count
    }

    /// Get number of withdrawals made
    pub fn get_withdrawal_count(&self) -> u32 {
        self.withdrawal_count
    }

    /// Get average deposit amount
    pub fn get_average_deposit(&self) -> f64 {
        if self.deposit_count == 0 {
            0.0
        } else {
            self.total_income as f64 / self.deposit_count as f64
        }
    }

    /// Get average withdrawal amount
    pub fn get_average_withdrawal(&self) -> f64 {
        if self.withdrawal_count == 0 {
            0.0
        } else {
            self.total_spent as f64 / self.withdrawal_count as f64
        }
    }

    /// Get time since last deposit
    pub fn time_since_last_deposit(&self) -> std::time::Duration {
        self.last_deposit_time.elapsed()
    }

    /// Get time since last withdrawal
    pub fn time_since_last_withdrawal(&self) -> std::time::Duration {
        self.last_withdrawal_time.elapsed()
    }

    /// Calculate income rate (money per second)
    pub fn calculate_income_rate(&self, time_window: std::time::Duration) -> f64 {
        let elapsed = self.last_deposit_time.elapsed();
        if elapsed < time_window {
            // Not enough time has passed for accurate rate calculation
            0.0
        } else {
            self.total_income as f64 / elapsed.as_secs_f64()
        }
    }

    /// Calculate spending rate (money per second)
    pub fn calculate_spending_rate(&self, time_window: std::time::Duration) -> f64 {
        let elapsed = self.last_withdrawal_time.elapsed();
        if elapsed < time_window {
            // Not enough time has passed for accurate rate calculation
            0.0
        } else {
            self.total_spent as f64 / elapsed.as_secs_f64()
        }
    }

    /// Transfer money from this account to another
    ///
    /// # Returns
    /// The amount actually transferred
    pub fn transfer_to(&mut self, other: &mut Money, amount: u32, play_sounds: bool) -> u32 {
        let actual_amount = self.withdraw(amount, play_sounds);
        if actual_amount > 0 {
            other.deposit(actual_amount, play_sounds);
        }
        actual_amount
    }

    // ========== Bounty System Integration ==========

    /// Award bounty for destroyed enemy unit
    ///
    /// This is typically called when an enemy unit is destroyed, awarding
    /// a percentage of its value to the player who destroyed it.
    ///
    /// # Arguments
    /// * `unit_value` - Base value of the destroyed unit
    /// * `bounty_percentage` - Percentage of value to award (0.0 to 1.0)
    /// * `play_sound` - Whether to play deposit sound
    ///
    /// # Returns
    /// The actual bounty amount awarded
    pub fn award_bounty(
        &mut self,
        unit_value: u32,
        bounty_percentage: f32,
        play_sound: bool,
    ) -> u32 {
        let bounty = (unit_value as f32 * bounty_percentage.clamp(0.0, 1.0)) as u32;
        if bounty > 0 {
            self.deposit(bounty, play_sound);
        }
        bounty
    }

    /// Award salvage bonus (money from destroyed buildings/units)
    ///
    /// # Arguments
    /// * `salvage_amount` - Amount of salvage to award
    /// * `play_sound` - Whether to play deposit sound
    pub fn award_salvage(&mut self, salvage_amount: u32, play_sound: bool) {
        if salvage_amount > 0 {
            self.deposit(salvage_amount, play_sound);
        }
    }

    // ========== Serialization Methods ==========

    /// Compute CRC for this Money instance
    ///
    /// Used for network synchronization and save game validation.
    ///
    /// # C++ Reference
    /// Money::crc() (Money.cpp lines 71-76)
    pub fn crc(&self) -> u32 {
        // C++ implementation is empty - CRC is computed at higher level
        // For Rust, we compute a simple hash of the money value
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.money.hash(&mut hasher);
        hasher.finish() as u32
    }

    /// Serialize/deserialize money data
    ///
    /// This method would be used with a proper Xfer system for save/load.
    ///
    /// # C++ Reference
    /// Money::xfer() (Money.cpp lines 78-94)
    ///
    /// # Version History
    /// * Version 1: Initial version (money value only)
    pub fn xfer_save(&self) -> Vec<u8> {
        // C++ Money.cpp lines 87-92: Save version and money value
        let mut data = Vec::new();

        // Version 1
        data.push(1u8);

        // Money value (u32 as 4 bytes)
        data.extend_from_slice(&self.money.to_le_bytes());

        data
    }

    /// Load money data from serialized bytes
    ///
    /// # C++ Reference
    /// Money::xfer() (Money.cpp lines 78-94)
    pub fn xfer_load(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() < 5 {
            return Err("Invalid money data: too short");
        }

        // C++ Money.cpp line 88-89: Read and validate version
        let version = data[0];
        if version != 1 {
            return Err("Invalid money version");
        }

        // C++ Money.cpp line 92: Read money value
        let money_bytes: [u8; 4] = data[1..5]
            .try_into()
            .map_err(|_| "Invalid money data format")?;
        self.money = u32::from_le_bytes(money_bytes);

        Ok(())
    }

    /// Post-load processing
    ///
    /// Called after deserialization to fix up any references or state.
    ///
    /// # C++ Reference
    /// Money::loadPostProcess() (Money.cpp lines 96-102)
    pub fn load_post_process(&mut self) {
        // C++ implementation is empty - no post-processing needed
        // Reset tracking stats since they're not serialized
        let now = std::time::Instant::now();
        self.last_deposit_time = now;
        self.last_withdrawal_time = now;
    }
}

impl Default for Money {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Money {
    fn clone(&self) -> Self {
        Self {
            money: self.money,
            player_index: self.player_index,
            total_income: self.total_income,
            total_spent: self.total_spent,
            deposit_count: self.deposit_count,
            withdrawal_count: self.withdrawal_count,
            last_deposit_time: self.last_deposit_time,
            last_withdrawal_time: self.last_withdrawal_time,
        }
    }
}

impl PartialEq for Money {
    fn eq(&self, other: &Self) -> bool {
        self.amount_equal(other)
    }
}

impl std::fmt::Display for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "${}", self.money)
    }
}

impl std::fmt::Debug for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Money")
            .field("money", &self.money)
            .field("player_index", &self.player_index)
            .finish()
    }
}

impl Money {
    /// Parse money amount from INI string
    ///
    /// # C++ Reference
    /// Money::parseMoneyAmount() (Money.cpp lines 106-113)
    ///
    /// # Note
    /// C++ comment says "Someday, maybe, have multiple fields like Gold:10000 Wood:1000 Tiberian:10"
    /// For now, just parses a simple unsigned integer value.
    pub fn parse_money_amount(value_str: &str) -> Result<u32, std::num::ParseIntError> {
        // C++ Money.cpp line 112: INI::parseUnsignedInt
        value_str.parse::<u32>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_money_creation() {
        let money = Money::new();
        assert_eq!(money.count_money(), 0);
        assert_eq!(money.get_player_index(), 0);

        let money_with_amount = Money::new_with_amount(1000);
        assert_eq!(money_with_amount.count_money(), 1000);
    }

    #[test]
    fn test_deposit_withdraw() {
        let mut money = Money::new();

        // Deposit some money
        money.deposit(1000, false);
        assert_eq!(money.count_money(), 1000);

        // Withdraw some money
        let withdrawn = money.withdraw(300, false);
        assert_eq!(withdrawn, 300);
        assert_eq!(money.count_money(), 700);

        // Try to withdraw more than we have
        let withdrawn = money.withdraw(1000, false);
        assert_eq!(withdrawn, 700);
        assert_eq!(money.count_money(), 0);
    }

    #[test]
    fn test_can_afford() {
        let mut money = Money::new();
        money.deposit(500, false);

        assert!(money.can_afford(300));
        assert!(money.can_afford(500));
        assert!(!money.can_afford(600));

        assert!(money.has_at_least(300));
        assert!(money.has_at_least(500));
        assert!(!money.has_at_least(600));
    }

    #[test]
    fn test_try_purchase() {
        let mut money = Money::new();
        money.deposit(500, false);

        // Successful purchase
        assert!(money.try_purchase(300, false));
        assert_eq!(money.count_money(), 200);

        // Failed purchase
        assert!(!money.try_purchase(300, false));
        assert_eq!(money.count_money(), 200);
    }

    #[test]
    fn test_transfer() {
        let mut money1 = Money::new_with_amount(500);
        let mut money2 = Money::new();

        let transferred = money1.transfer_to(&mut money2, 300, false);

        assert_eq!(transferred, 300);
        assert_eq!(money1.count_money(), 200);
        assert_eq!(money2.count_money(), 300);
    }

    #[test]
    fn test_percentage() {
        let money = Money::new_with_amount(250);

        assert_eq!(money.get_percentage_of(1000), 25.0);
        assert_eq!(money.get_percentage_of(250), 100.0);
        assert_eq!(money.get_percentage_of(0), 0.0);
    }

    #[test]
    fn test_equality() {
        let money1 = Money::new_with_amount(500);
        let mut money2 = Money::new_with_amount(500);
        let money3 = Money::new_with_amount(300);

        assert_eq!(money1, money2);
        assert_ne!(money1, money3);

        // Player index should not affect equality
        money2.set_player_index(5);
        assert_eq!(money1, money2);
    }

    #[test]
    fn test_display() {
        let money = Money::new_with_amount(1500);
        assert_eq!(format!("{}", money), "$1500");
    }

    #[test]
    fn test_income_tracking() {
        let mut money = Money::new();

        // Make some deposits
        money.deposit(1000, false);
        money.deposit(500, false);
        money.deposit(250, false);

        assert_eq!(money.get_total_income(), 1750);
        assert_eq!(money.get_deposit_count(), 3);
        assert_eq!(money.get_average_deposit(), 583.3333333333334);
    }

    #[test]
    fn test_spending_tracking() {
        let mut money = Money::new_with_amount(2000);

        // Make some withdrawals
        money.withdraw(500, false);
        money.withdraw(300, false);
        money.withdraw(200, false);

        assert_eq!(money.get_total_spent(), 1000);
        assert_eq!(money.get_withdrawal_count(), 3);
        assert_eq!(money.get_average_withdrawal(), 333.3333333333333);
        assert_eq!(money.count_money(), 1000);
    }

    #[test]
    fn test_net_income() {
        let mut money = Money::new();

        money.deposit(5000, false);
        money.withdraw(2000, false);
        money.deposit(1000, false);
        money.withdraw(500, false);

        assert_eq!(money.get_total_income(), 6000);
        assert_eq!(money.get_total_spent(), 2500);
        assert_eq!(money.get_net_income(), 3500);
        assert_eq!(money.count_money(), 3500);
    }

    #[test]
    fn test_bounty_system() {
        let mut money = Money::new();

        // Award 25% bounty for a unit worth 1000
        let bounty = money.award_bounty(1000, 0.25, false);
        assert_eq!(bounty, 250);
        assert_eq!(money.count_money(), 250);

        // Award 50% bounty for a unit worth 800
        let bounty = money.award_bounty(800, 0.5, false);
        assert_eq!(bounty, 400);
        assert_eq!(money.count_money(), 650);

        // Test clamping - 150% should be clamped to 100%
        let bounty = money.award_bounty(100, 1.5, false);
        assert_eq!(bounty, 100);
        assert_eq!(money.count_money(), 750);

        // Test negative percentage - should be clamped to 0%
        let bounty = money.award_bounty(100, -0.5, false);
        assert_eq!(bounty, 0);
        assert_eq!(money.count_money(), 750);
    }

    #[test]
    fn test_salvage_award() {
        let mut money = Money::new();

        money.award_salvage(500, false);
        assert_eq!(money.count_money(), 500);

        money.award_salvage(300, false);
        assert_eq!(money.count_money(), 800);

        // Zero salvage should do nothing
        money.award_salvage(0, false);
        assert_eq!(money.count_money(), 800);
    }

    #[test]
    fn test_serialization() {
        let mut money = Money::new_with_amount(12345);
        money.set_player_index(2);

        // Test save
        let data = money.xfer_save();
        assert_eq!(data.len(), 5); // 1 byte version + 4 bytes u32

        // Test load
        let mut loaded_money = Money::new();
        let result = loaded_money.xfer_load(&data);
        assert!(result.is_ok());
        assert_eq!(loaded_money.count_money(), 12345);

        // Test post-processing
        loaded_money.load_post_process();
        assert_eq!(loaded_money.count_money(), 12345);
    }

    #[test]
    fn test_serialization_invalid_data() {
        let mut money = Money::new();

        // Test with too short data
        let result = money.xfer_load(&[1, 2, 3]);
        assert!(result.is_err());

        // Test with invalid version
        let result = money.xfer_load(&[99, 0, 0, 0, 0]);
        assert!(result.is_err());
    }

    #[test]
    fn test_crc() {
        let money1 = Money::new_with_amount(1000);
        let money2 = Money::new_with_amount(1000);
        let money3 = Money::new_with_amount(2000);

        // Same amounts should produce same CRC
        assert_eq!(money1.crc(), money2.crc());

        // Different amounts should (probably) produce different CRC
        assert_ne!(money1.crc(), money3.crc());
    }

    #[test]
    fn test_parse_money_amount() {
        assert_eq!(Money::parse_money_amount("10000").unwrap(), 10000);
        assert_eq!(Money::parse_money_amount("0").unwrap(), 0);
        assert_eq!(Money::parse_money_amount("999999").unwrap(), 999999);

        // Invalid input should fail
        assert!(Money::parse_money_amount("not a number").is_err());
        assert!(Money::parse_money_amount("-100").is_err());
    }

    #[test]
    fn test_time_tracking() {
        let mut money = Money::new();

        money.deposit(1000, false);
        std::thread::sleep(std::time::Duration::from_millis(10));

        let time_since_deposit = money.time_since_last_deposit();
        assert!(time_since_deposit.as_millis() >= 10);

        money.withdraw(500, false);
        std::thread::sleep(std::time::Duration::from_millis(10));

        let time_since_withdrawal = money.time_since_last_withdrawal();
        assert!(time_since_withdrawal.as_millis() >= 10);
    }

    #[test]
    fn test_rate_calculations() {
        let mut money = Money::new();

        // Make some deposits
        money.deposit(1000, false);
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Rate calculation with short window
        let income_rate = money.calculate_income_rate(std::time::Duration::from_millis(50));
        assert!(income_rate > 0.0);

        // Make some withdrawals
        money.withdraw(500, false);
        std::thread::sleep(std::time::Duration::from_millis(100));

        let spending_rate = money.calculate_spending_rate(std::time::Duration::from_millis(50));
        assert!(spending_rate > 0.0);
    }

    #[test]
    fn test_init_resets_tracking() {
        let mut money = Money::new();

        money.deposit(1000, false);
        money.withdraw(500, false);

        assert_eq!(money.count_money(), 500);
        assert_eq!(money.get_total_income(), 1000);
        assert_eq!(money.get_total_spent(), 500);

        // Init should reset everything
        money.init();
        assert_eq!(money.count_money(), 0);
        assert_eq!(money.get_total_income(), 0);
        assert_eq!(money.get_total_spent(), 0);
        assert_eq!(money.get_deposit_count(), 0);
        assert_eq!(money.get_withdrawal_count(), 0);
    }

    #[test]
    fn test_clone_preserves_state() {
        let mut money = Money::new_with_amount(1000);
        money.set_player_index(5);
        money.deposit(500, false);

        let cloned = money.clone();

        assert_eq!(cloned.count_money(), money.count_money());
        assert_eq!(cloned.get_player_index(), money.get_player_index());
        // Note: tracking stats are preserved in clone but not player_index in equality
        assert_eq!(cloned, money);
    }
}
