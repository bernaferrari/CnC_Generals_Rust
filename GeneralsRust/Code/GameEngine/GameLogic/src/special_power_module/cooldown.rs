//! Cooldown Management System for Special Powers

use crate::common::*;
use std::collections::HashMap;

/// Cooldown state for a special power
#[derive(Debug, Clone)]
pub struct CooldownState {
    /// Total cooldown duration in seconds
    pub cooldown_duration: Real,
    /// Time remaining on cooldown in seconds
    pub time_remaining: Real,
    /// Initial charge time (first use) in seconds
    pub initial_charge_time: Real,
    /// Frame when cooldown started
    pub cooldown_start_frame: UnsignedInt,
    /// Whether this is the first activation
    pub is_first_activation: Bool,
    /// Whether cooldown is paused
    pub paused: Bool,
}

impl CooldownState {
    pub fn new(cooldown_duration: Real, initial_charge_time: Real) -> Self {
        let is_first_activation = initial_charge_time > 0.0;
        let time_remaining = if is_first_activation {
            initial_charge_time
        } else {
            0.0
        };

        Self {
            cooldown_duration,
            time_remaining,
            initial_charge_time,
            cooldown_start_frame: 0,
            is_first_activation,
            paused: false,
        }
    }

    /// Check if the power is ready (cooldown complete)
    pub fn is_ready(&self) -> Bool {
        self.time_remaining <= 0.0 && !self.paused
    }

    /// Check if the power is on cooldown
    pub fn is_on_cooldown(&self) -> Bool {
        self.time_remaining > 0.0
    }

    /// Get progress as a percentage (0.0 to 1.0)
    pub fn get_progress(&self) -> Real {
        if self.cooldown_duration <= 0.0 {
            return 1.0;
        }

        let total_time = if self.is_first_activation && self.initial_charge_time > 0.0 {
            self.initial_charge_time
        } else {
            self.cooldown_duration
        };

        if total_time <= 0.0 {
            return 1.0;
        }

        1.0 - (self.time_remaining / total_time).clamp(0.0, 1.0)
    }

    /// Start cooldown after activation
    pub fn start_cooldown(&mut self, current_frame: UnsignedInt) {
        self.cooldown_start_frame = current_frame;
        self.time_remaining = self.cooldown_duration;
        self.is_first_activation = false;
    }

    /// Update cooldown state (call every frame)
    pub fn update(&mut self, delta_time: Real) {
        if self.paused || self.time_remaining <= 0.0 {
            return;
        }

        self.time_remaining -= delta_time;
        if self.time_remaining < 0.0 {
            self.time_remaining = 0.0;
        }
    }

    /// Reset cooldown (make power available immediately)
    pub fn reset(&mut self) {
        self.time_remaining = 0.0;
        self.paused = false;
    }

    /// Set remaining time directly
    pub fn set_remaining_time(&mut self, time: Real) {
        self.time_remaining = time.max(0.0);
    }

    /// Add time to cooldown
    pub fn add_time(&mut self, time: Real) {
        self.time_remaining += time;
    }

    /// Reduce cooldown time (e.g., from upgrades)
    pub fn reduce_cooldown(&mut self, amount: Real) {
        self.cooldown_duration = (self.cooldown_duration - amount).max(0.0);
        if self.time_remaining > self.cooldown_duration {
            self.time_remaining = self.cooldown_duration;
        }
    }

    /// Pause cooldown
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume cooldown
    pub fn resume(&mut self) {
        self.paused = false;
    }
}

/// Cooldown manager for tracking multiple special powers
#[derive(Debug)]
pub struct CooldownManager {
    cooldowns: HashMap<SpecialPowerID, CooldownState>,
    shared_groups: HashMap<AsciiString, Vec<SpecialPowerID>>,
}

impl CooldownManager {
    pub fn new() -> Self {
        Self {
            cooldowns: HashMap::new(),
            shared_groups: HashMap::new(),
        }
    }

    /// Register a special power with its cooldown
    pub fn register_power(
        &mut self,
        power_id: SpecialPowerID,
        cooldown_duration: Real,
        initial_charge_time: Real,
        shared_group: Option<AsciiString>,
    ) {
        let state = CooldownState::new(cooldown_duration, initial_charge_time);
        self.cooldowns.insert(power_id, state);

        // Add to shared group if specified
        if let Some(group) = shared_group {
            self.shared_groups
                .entry(group)
                .or_insert_with(Vec::new)
                .push(power_id);
        }
    }

    /// Get cooldown state for a power
    pub fn get_state(&self, power_id: SpecialPowerID) -> Option<&CooldownState> {
        self.cooldowns.get(&power_id)
    }

    /// Get mutable cooldown state for a power
    pub fn get_state_mut(&mut self, power_id: SpecialPowerID) -> Option<&mut CooldownState> {
        self.cooldowns.get_mut(&power_id)
    }

    /// Check if power is ready
    pub fn is_ready(&self, power_id: SpecialPowerID) -> Bool {
        self.cooldowns
            .get(&power_id)
            .map(|s| s.is_ready())
            .unwrap_or(false)
    }

    /// Start cooldown for a power (and all powers in its shared group)
    pub fn start_cooldown(&mut self, power_id: SpecialPowerID, current_frame: UnsignedInt) {
        // Start cooldown for the power itself
        if let Some(state) = self.cooldowns.get_mut(&power_id) {
            state.start_cooldown(current_frame);
        }

        // Start cooldown for all powers in the same shared group
        let shared_powers: Vec<SpecialPowerID> = self
            .shared_groups
            .values()
            .filter(|powers| powers.contains(&power_id))
            .flat_map(|powers| powers.clone())
            .filter(|&id| id != power_id)
            .collect();

        for shared_id in shared_powers {
            if let Some(state) = self.cooldowns.get_mut(&shared_id) {
                state.start_cooldown(current_frame);
            }
        }
    }

    /// Update all cooldowns
    pub fn update(&mut self, delta_time: Real) {
        for state in self.cooldowns.values_mut() {
            state.update(delta_time);
        }
    }

    /// Reset a specific power's cooldown
    pub fn reset_power(&mut self, power_id: SpecialPowerID) {
        if let Some(state) = self.cooldowns.get_mut(&power_id) {
            state.reset();
        }
    }

    /// Reset all cooldowns
    pub fn reset_all(&mut self) {
        for state in self.cooldowns.values_mut() {
            state.reset();
        }
    }

    /// Get all powers in a shared group
    pub fn get_shared_group(&self, group_name: &AsciiString) -> Option<&Vec<SpecialPowerID>> {
        self.shared_groups.get(group_name)
    }
}

impl Default for CooldownManager {
    fn default() -> Self {
        Self::new()
    }
}

use super::types::SpecialPowerID;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cooldown_state() {
        let mut state = CooldownState::new(30.0, 10.0);

        // Should start with initial charge time
        assert!(state.is_first_activation);
        assert_eq!(state.time_remaining, 10.0);
        assert!(!state.is_ready());

        // Update should reduce time
        state.update(5.0);
        assert_eq!(state.time_remaining, 5.0);

        state.update(5.0);
        assert_eq!(state.time_remaining, 0.0);
        assert!(state.is_ready());

        // After activation, should use normal cooldown
        state.start_cooldown(100);
        assert!(!state.is_first_activation);
        assert_eq!(state.time_remaining, 30.0);
        assert!(!state.is_ready());
    }

    #[test]
    fn test_cooldown_progress() {
        let mut state = CooldownState::new(30.0, 0.0);
        assert_eq!(state.get_progress(), 1.0);

        state.start_cooldown(0);
        assert_eq!(state.get_progress(), 0.0);

        state.update(15.0);
        assert!((state.get_progress() - 0.5).abs() < 0.001);

        state.update(15.0);
        assert_eq!(state.get_progress(), 1.0);
    }

    #[test]
    fn test_cooldown_manager() {
        let mut manager = CooldownManager::new();

        // Register powers
        manager.register_power(1, 30.0, 10.0, Some("group1".into()));
        manager.register_power(2, 30.0, 10.0, Some("group1".into()));
        manager.register_power(3, 45.0, 0.0, None);

        // Check initial state
        assert!(!manager.is_ready(1)); // Has initial charge time
        assert!(!manager.is_ready(2));
        assert!(manager.is_ready(3)); // No initial charge

        // Update cooldowns
        manager.update(10.0);
        assert!(manager.is_ready(1));
        assert!(manager.is_ready(2));

        // Start cooldown should affect shared group
        manager.start_cooldown(1, 100);
        assert!(!manager.is_ready(1));
        assert!(!manager.is_ready(2)); // Also affected by shared group
        assert!(manager.is_ready(3)); // Not in shared group
    }

    #[test]
    fn test_cooldown_pause() {
        let mut state = CooldownState::new(30.0, 0.0);
        state.start_cooldown(0);

        state.update(10.0);
        assert_eq!(state.time_remaining, 20.0);

        state.pause();
        state.update(10.0);
        assert_eq!(state.time_remaining, 20.0); // Should not decrease

        state.resume();
        state.update(10.0);
        assert_eq!(state.time_remaining, 10.0); // Should decrease again
    }
}
