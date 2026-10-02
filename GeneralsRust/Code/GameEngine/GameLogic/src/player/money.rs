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

    /// Set money to an exact amount (matching C++ Player::setMoney)
    pub fn set_money(&mut self, amount: Int) {
        self.amount = amount;
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

    /// Returns the currently available cash (non-negative) as an unsigned amount.
    pub fn count_money(&self) -> u32 {
        self.amount.max(0) as u32
    }
}

impl MoneyInterface for PlayerMoney {
    fn count_money(&self) -> i32 {
        self.amount
    }
}

/// Money operations execute through the player that owns both cash and statistics.
/// This borrow cannot escape as mutable cash storage or rediscover its owner.
pub struct PlayerMoneyMut<'a> {
    player: &'a mut Player,
    locality: Option<game_engine::common::audio::AudioSubmissionLocality>,
}

impl<'a> PlayerMoneyMut<'a> {
    pub(super) fn new(player: &'a mut Player) -> Self {
        Self {
            player,
            locality: None,
        }
    }

    pub(super) fn with_locality(
        player: &'a mut Player,
        locality: game_engine::common::audio::AudioSubmissionLocality,
    ) -> Self {
        Self {
            player,
            locality: Some(locality),
        }
    }

    pub fn set_money(&mut self, amount: Int) {
        self.player.money.set_money(amount);
    }
    pub fn set_income_rate(&mut self, rate: Real) {
        self.player.money.set_income_rate(rate);
    }
    pub fn add_money(&mut self, amount: Int) {
        if amount >= 0 {
            let _ = self.deposit(amount as u32);
        } else {
            let _ = self.withdraw(amount.unsigned_abs());
        }
    }

    pub fn subtract_money(&mut self, amount: Int) -> bool {
        if amount <= 0 {
            return true;
        }
        if self.player.money.amount < amount {
            return false;
        }
        let _ = self.withdraw(amount as u32);
        true
    }

    fn submit_sound(&self, deposit: bool) {
        let Some(audio) = crate::helpers::TheAudio::get() else {
            return;
        };
        let misc = crate::helpers::TheAudio::get_misc_audio();
        let mut event = if deposit {
            misc.money_deposit
        } else {
            misc.money_withdraw
        };
        event.set_player_index(self.player.money.player_index as u32);
        let locality = match self.locality {
            Some(locality) => locality,
            None => {
                // Temporary classic adapter: discover the roster once, before
                // borrowing audio. Callers already holding it pass frozen facts.
                let Ok(list) = player_list().read() else {
                    return;
                };
                crate::helpers::capture_player_audio_locality(self.player, &list)
            }
        };
        audio.add_audio_event_with_locality(&event, &locality);
    }

    pub fn withdraw(&mut self, amount: u32) -> Result<u32, GameError> {
        self.withdraw_with_sound(amount, true)
    }

    /// C++ Money.cpp: clamp, zero return, submit sound, then debit.
    pub fn withdraw_with_sound(&mut self, amount: u32, play_sound: bool) -> Result<u32, GameError> {
        self.withdraw_with_submission(amount, play_sound, |money| money.submit_sound(false))
    }

    fn withdraw_with_submission(
        &mut self,
        amount: u32,
        play_sound: bool,
        submit: impl FnOnce(&Self),
    ) -> Result<u32, GameError> {
        let actual = amount.min(self.player.money.count_money());
        if actual == 0 {
            return Ok(0);
        }
        if play_sound {
            submit(self);
        }
        self.player.money.amount = self.player.money.amount.saturating_sub(actual as Int);
        Ok(actual)
    }

    pub fn deposit(&mut self, amount: u32) -> Result<(), GameError> {
        self.deposit_with_sound(amount, true)
    }

    /// C++ Money.cpp: zero return, submit sound, credit, then record income.
    pub fn deposit_with_sound(&mut self, amount: u32, play_sound: bool) -> Result<(), GameError> {
        self.deposit_with_submission(amount, play_sound, |money| money.submit_sound(true))
    }

    fn deposit_with_submission(
        &mut self,
        amount: u32,
        play_sound: bool,
        submit: impl FnOnce(&Self),
    ) -> Result<(), GameError> {
        if amount == 0 {
            return Ok(());
        }
        if play_sound {
            submit(self);
        }
        self.player.money.amount = self.player.money.amount.saturating_add(amount as Int);
        self.player.academy_stats.record_income(amount as Int);
        Ok(())
    }

    /// Existing direct setter interface; no sound or income callback.
    pub fn deposit_money(&mut self, amount: Int) {
        self.player.money.amount = self.player.money.amount.saturating_add(amount);
    }

    pub fn add_money_earned(&mut self, amount: Int) {
        if amount > 0 {
            self.player.score_keeper.add_money_earned(amount as u32);
        }
    }
}

impl std::ops::Deref for PlayerMoneyMut<'_> {
    type Target = PlayerMoney;
    fn deref(&self) -> &Self::Target {
        &self.player.money
    }
}

#[cfg(test)]
#[path = "money_tests.rs"]
mod tests;
