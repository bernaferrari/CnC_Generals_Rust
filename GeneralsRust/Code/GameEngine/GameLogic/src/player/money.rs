use super::*;

/// Player money/resource management (matching C++ Money class)
#[derive(Debug, Clone)]
pub struct PlayerMoney {
    pub(super) amount: Int,
    pub(super) income_rate: Real,
    pub(super) last_update_frame: UnsignedInt,
    pub(super) player_index: PlayerIndex,
}

impl PlayerMoney {
    pub fn new(player_index: PlayerIndex) -> Self {
        Self {
            amount: 0,
            income_rate: 0.0,
            last_update_frame: 0,
            player_index,
        }
    }

    pub fn get_money(&self) -> Int {
        self.amount
    }

    pub fn add_money(&mut self, amount: Int) {
        if amount >= 0 {
            let _ = self.deposit(amount as u32);
        } else {
            let _ = self.withdraw((-amount) as u32);
        }
    }

    /// Set money to an exact amount (matching C++ Player::setMoney)
    pub fn set_money(&mut self, amount: Int) {
        self.amount = amount;
    }

    pub fn subtract_money(&mut self, amount: Int) -> bool {
        if amount <= 0 {
            return true;
        }
        if self.amount >= amount {
            let _ = self.withdraw(amount as u32);
            true
        } else {
            false
        }
    }

    pub fn can_afford(&self, cost: Int) -> bool {
        self.amount >= cost
    }

    pub fn set_income_rate(&mut self, rate: Real) {
        self.income_rate = rate;
    }

    pub fn get_income_rate(&self) -> Real {
        self.income_rate
    }

    pub fn set_player_index(&mut self, player_index: PlayerIndex) {
        self.player_index = player_index;
    }

    /// Returns the currently available cash (non-negative) as an unsigned amount.
    pub fn count_money(&self) -> u32 {
        self.amount.max(0) as u32
    }

    /// Withdraw money from the player's reserves.
    pub fn withdraw(&mut self, amount: u32) -> Result<u32, GameError> {
        self.withdraw_with_sound(amount, true)
    }

    /// Withdraw money from the player's reserves, optionally playing a sound.
    /// Matches C++ Money::withdraw(amount, playSound).
    pub fn withdraw_with_sound(&mut self, amount: u32, play_sound: bool) -> Result<u32, GameError> {
        let available = self.count_money();
        let actual = amount.min(available);
        if actual == 0 {
            return Ok(0);
        }

        if play_sound {
            if let Some(audio) = crate::helpers::TheAudio::get() {
                let mut audio_event = crate::helpers::TheAudio::get_misc_audio()
                    .money_withdraw
                    .clone();
                audio_event.set_player_index(self.player_index as u32);
                audio.add_audio_event(&audio_event);
            }
        }

        self.amount = self.amount.saturating_sub(actual as Int);
        Ok(actual)
    }

    /// Deposit money into the player's reserves.
    pub fn deposit(&mut self, amount: u32) -> Result<(), GameError> {
        self.deposit_with_sound(amount, true)
    }

    /// Deposit money into the player's reserves, optionally playing a sound.
    /// Matches C++ Money::deposit(amount, playSound).
    pub fn deposit_with_sound(&mut self, amount: u32, play_sound: bool) -> Result<(), GameError> {
        if amount == 0 {
            return Ok(());
        }

        if play_sound {
            if let Some(audio) = crate::helpers::TheAudio::get() {
                let mut audio_event = crate::helpers::TheAudio::get_misc_audio()
                    .money_deposit
                    .clone();
                audio_event.set_player_index(self.player_index as u32);
                audio.add_audio_event(&audio_event);
            }
        }

        self.amount = self.amount.saturating_add(amount as Int);
        if let Ok(list) = player_list().read() {
            if let Some(player) = list.get_player(self.player_index) {
                if let Ok(mut player_guard) = player.write() {
                    player_guard
                        .get_academy_stats_mut()
                        .record_income(amount as Int);
                }
            }
        }
        Ok(())
    }

    /// Deposit money from Int amount (alternative interface).
    pub fn deposit_money(&mut self, amount: Int) {
        self.amount = self.amount.saturating_add(amount);
    }

    /// Track money earned for statistics (currently just adds to total).
    pub fn add_money_earned(&mut self, amount: Int) {
        if amount <= 0 {
            return;
        }

        if let Ok(list) = player_list().read() {
            if let Some(player) = list.get_player(self.player_index) {
                if let Ok(mut player_guard) = player.write() {
                    player_guard.score_keeper.add_money_earned(amount as u32);
                }
            }
        }
    }
}

impl MoneyInterface for PlayerMoney {
    fn count_money(&self) -> i32 {
        self.amount
    }
}
