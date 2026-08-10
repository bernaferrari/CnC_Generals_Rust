// ============================================================================
// MONEY SYSTEM
// ============================================================================

/// Player's money account
/// Matches C++ Money.cpp
pub struct Money {
    /// Current money amount
    money: u32,
    /// Player index
    player_index: PlayerIndex,
    /// Audio system reference (optional)
    audio_system: Option<Arc<dyn AudioSystem>>,
    /// Academy stats reference (optional)
    academy_stats: Option<Arc<dyn AcademyStats>>,
    /// Total money earned (for statistics)
    total_earned: u32,
    /// Total money spent (for statistics)
    total_spent: u32,
    /// Bounty from destroyed enemy units
    bounty_earned: u32,
    /// Salvage from crates and pickups
    salvage_earned: u32,
}

impl std::fmt::Debug for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Money")
            .field("money", &self.money)
            .field("player_index", &self.player_index)
            .field("total_earned", &self.total_earned)
            .field("total_spent", &self.total_spent)
            .field("bounty_earned", &self.bounty_earned)
            .field("salvage_earned", &self.salvage_earned)
            .field(
                "audio_system",
                &self.audio_system.as_ref().map(|_| "AudioSystem"),
            )
            .field(
                "academy_stats",
                &self.academy_stats.as_ref().map(|_| "AcademyStats"),
            )
            .finish()
    }
}

impl Money {
    pub fn new(player_index: PlayerIndex, starting_money: u32) -> Self {
        Self {
            money: starting_money,
            player_index,
            audio_system: None,
            academy_stats: None,
            total_earned: starting_money,
            total_spent: 0,
            bounty_earned: 0,
            salvage_earned: 0,
        }
    }

    /// Set audio system for sound playback
    pub fn set_audio_system(&mut self, audio_system: Arc<dyn AudioSystem>) {
        self.audio_system = Some(audio_system);
    }

    /// Set academy stats for income tracking
    pub fn set_academy_stats(&mut self, academy_stats: Arc<dyn AcademyStats>) {
        self.academy_stats = Some(academy_stats);
    }

    /// Withdraw money from account
    /// Matches C++ Money::withdraw() - Money.cpp:23
    pub fn withdraw(&mut self, amount_to_withdraw: u32, play_sound: bool) -> u32 {
        let actual_amount = if amount_to_withdraw > self.money {
            self.money
        } else {
            amount_to_withdraw
        };

        if actual_amount == 0 {
            return 0;
        }

        // Play sound if enabled
        // Matches C++ Money.cpp:32-37
        if play_sound {
            if let Some(audio) = &self.audio_system {
                audio.play_money_withdraw_sound(self.player_index);
            }
        }

        self.money -= actual_amount;
        self.total_spent += actual_amount;
        actual_amount
    }

    /// Deposit money into account
    /// Matches C++ Money::deposit() - Money.cpp:45
    pub fn deposit(&mut self, amount_to_deposit: u32, play_sound: bool) {
        if amount_to_deposit == 0 {
            return;
        }

        // Play sound if enabled
        // Matches C++ Money.cpp:51-56
        if play_sound {
            if let Some(audio) = &self.audio_system {
                audio.play_money_deposit_sound(self.player_index);
            }
        }

        self.money += amount_to_deposit;
        self.total_earned += amount_to_deposit;

        // Record income for academy stats
        // Matches C++ Money.cpp:60-67
        if amount_to_deposit > 0 {
            if let Some(stats) = &self.academy_stats {
                stats.record_income();
            }
        }
    }

    pub fn get_money(&self) -> u32 {
        self.money
    }

    pub fn set_money(&mut self, amount: u32) {
        self.money = amount;
    }

    pub fn can_afford(&self, cost: u32) -> bool {
        self.money >= cost
    }

    /// Award bounty for destroying enemy unit
    /// Bounty system matches C++ kill/bounty mechanics
    pub fn award_bounty(&mut self, bounty_amount: u32) {
        if bounty_amount > 0 {
            self.deposit(bounty_amount, false);
            self.bounty_earned += bounty_amount;
        }
    }

    /// Award salvage from crate pickup
    /// Crate system matches C++ MoneyCrateCollide.cpp
    pub fn award_salvage(&mut self, salvage_amount: u32) {
        if salvage_amount > 0 {
            self.deposit(salvage_amount, true);
            self.salvage_earned += salvage_amount;
        }
    }

    pub fn get_total_earned(&self) -> u32 {
        self.total_earned
    }

    pub fn get_total_spent(&self) -> u32 {
        self.total_spent
    }

    pub fn get_bounty_earned(&self) -> u32 {
        self.bounty_earned
    }

    pub fn get_salvage_earned(&self) -> u32 {
        self.salvage_earned
    }
}

