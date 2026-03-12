// FILE: special_power_cooldown.rs
// Port of Special Power cooldown management
// Author: Rust Port
// Desc: Per-power and shared cooldown group management

use crate::object::special_power_module::{CooldownGroup, FrameCount};
use crate::object::special_power_types::SpecialPowerType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cooldown state for a specific power or group
#[derive(Debug, Clone)]
pub struct CooldownState {
    /// Frame when cooldown started
    pub start_frame: FrameCount,
    /// Frame when power becomes ready
    pub ready_frame: FrameCount,
    /// Is cooldown active
    pub active: bool,
}

impl CooldownState {
    /// Create a new inactive cooldown state
    pub fn new_inactive() -> Self {
        Self {
            start_frame: 0,
            ready_frame: 0,
            active: false,
        }
    }

    /// Create an active cooldown state
    pub fn new_active(current_frame: FrameCount, duration: FrameCount) -> Self {
        Self {
            start_frame: current_frame,
            ready_frame: current_frame + duration,
            active: true,
        }
    }

    /// Check if ready at the given frame
    pub fn is_ready(&self, current_frame: FrameCount) -> bool {
        !self.active || current_frame >= self.ready_frame
    }

    /// Get remaining cooldown frames
    pub fn remaining_frames(&self, current_frame: FrameCount) -> FrameCount {
        if self.is_ready(current_frame) {
            0
        } else {
            self.ready_frame.saturating_sub(current_frame)
        }
    }

    /// Get cooldown progress (0.0 = just started, 1.0 = ready)
    pub fn get_progress(&self, current_frame: FrameCount) -> f32 {
        if !self.active {
            return 1.0;
        }

        if current_frame >= self.ready_frame {
            return 1.0;
        }

        let total_duration = self.ready_frame - self.start_frame;
        if total_duration == 0 {
            return 1.0;
        }

        let elapsed = current_frame - self.start_frame;
        elapsed as f32 / total_duration as f32
    }

    /// Reset cooldown to inactive
    pub fn reset(&mut self) {
        self.active = false;
        self.start_frame = 0;
        self.ready_frame = 0;
    }

    /// Start a new cooldown
    pub fn start(&mut self, current_frame: FrameCount, duration: FrameCount) {
        self.start_frame = current_frame;
        self.ready_frame = current_frame + duration;
        self.active = true;
    }
}

/// Player-wide cooldown manager
/// Manages individual power cooldowns and shared cooldown groups
/// Matches C++ Player special power management
#[derive(Debug, Clone)]
pub struct CooldownManager {
    /// Cooldowns for individual powers (indexed by power type)
    power_cooldowns: HashMap<SpecialPowerType, CooldownState>,

    /// Cooldowns for shared groups
    group_cooldowns: HashMap<CooldownGroup, CooldownState>,

    /// Mapping of powers to their cooldown group
    power_to_group: HashMap<SpecialPowerType, CooldownGroup>,
}

impl Default for CooldownManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CooldownManager {
    /// Create a new cooldown manager
    pub fn new() -> Self {
        Self {
            power_cooldowns: HashMap::new(),
            group_cooldowns: HashMap::new(),
            power_to_group: HashMap::new(),
        }
    }

    /// Register a power with its cooldown group
    pub fn register_power(&mut self, power_type: SpecialPowerType, group: CooldownGroup) {
        self.power_to_group.insert(power_type, group);
        self.power_cooldowns
            .insert(power_type, CooldownState::new_inactive());

        // Ensure group cooldown exists
        if group != CooldownGroup::None {
            self.group_cooldowns
                .entry(group)
                .or_insert_with(CooldownState::new_inactive);
        }
    }

    /// Check if a power is ready (not on cooldown)
    /// Matches C++ behavior: checks both individual and group cooldowns
    pub fn is_power_ready(&self, power_type: SpecialPowerType, current_frame: FrameCount) -> bool {
        // Check individual power cooldown
        if let Some(cooldown) = self.power_cooldowns.get(&power_type) {
            if !cooldown.is_ready(current_frame) {
                return false;
            }
        }

        // Check shared group cooldown
        if let Some(&group) = self.power_to_group.get(&power_type) {
            if group != CooldownGroup::None {
                if let Some(group_cooldown) = self.group_cooldowns.get(&group) {
                    if !group_cooldown.is_ready(current_frame) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Start cooldown for a power
    /// Matches C++ behavior: sets both individual and group cooldowns
    pub fn start_cooldown(
        &mut self,
        power_type: SpecialPowerType,
        current_frame: FrameCount,
        duration: FrameCount,
    ) {
        // Start individual power cooldown
        self.power_cooldowns
            .entry(power_type)
            .or_insert_with(CooldownState::new_inactive)
            .start(current_frame, duration);

        // Start shared group cooldown if applicable
        if let Some(&group) = self.power_to_group.get(&power_type) {
            if group != CooldownGroup::None {
                self.group_cooldowns
                    .entry(group)
                    .or_insert_with(CooldownState::new_inactive)
                    .start(current_frame, duration);
            }
        }
    }

    /// Get remaining cooldown frames for a power
    pub fn get_remaining_cooldown(
        &self,
        power_type: SpecialPowerType,
        current_frame: FrameCount,
    ) -> FrameCount {
        let mut max_remaining = 0;

        // Check individual cooldown
        if let Some(cooldown) = self.power_cooldowns.get(&power_type) {
            max_remaining = max_remaining.max(cooldown.remaining_frames(current_frame));
        }

        // Check group cooldown
        if let Some(&group) = self.power_to_group.get(&power_type) {
            if group != CooldownGroup::None {
                if let Some(group_cooldown) = self.group_cooldowns.get(&group) {
                    max_remaining =
                        max_remaining.max(group_cooldown.remaining_frames(current_frame));
                }
            }
        }

        max_remaining
    }

    /// Get cooldown progress for a power (0.0 = just started, 1.0 = ready)
    pub fn get_progress(&self, power_type: SpecialPowerType, current_frame: FrameCount) -> f32 {
        let mut min_progress: f32 = 1.0;

        // Check individual cooldown
        if let Some(cooldown) = self.power_cooldowns.get(&power_type) {
            min_progress = min_progress.min(cooldown.get_progress(current_frame) as f32);
        }

        // Check group cooldown
        if let Some(&group) = self.power_to_group.get(&power_type) {
            if group != CooldownGroup::None {
                if let Some(group_cooldown) = self.group_cooldowns.get(&group) {
                    min_progress =
                        min_progress.min(group_cooldown.get_progress(current_frame) as f32);
                }
            }
        }

        min_progress
    }

    /// Reset a specific power cooldown (for cheats/testing)
    pub fn reset_power_cooldown(&mut self, power_type: SpecialPowerType) {
        if let Some(cooldown) = self.power_cooldowns.get_mut(&power_type) {
            cooldown.reset();
        }
    }

    /// Reset a cooldown group (affects all powers in that group)
    pub fn reset_group_cooldown(&mut self, group: CooldownGroup) {
        if let Some(cooldown) = self.group_cooldowns.get_mut(&group) {
            cooldown.reset();
        }
    }

    /// Reset all cooldowns (for testing or special events)
    pub fn reset_all_cooldowns(&mut self) {
        for cooldown in self.power_cooldowns.values_mut() {
            cooldown.reset();
        }
        for cooldown in self.group_cooldowns.values_mut() {
            cooldown.reset();
        }
    }

    /// Get all powers in a cooldown group
    pub fn get_powers_in_group(&self, group: CooldownGroup) -> Vec<SpecialPowerType> {
        self.power_to_group
            .iter()
            .filter(|(_, &g)| g == group)
            .map(|(&power, _)| power)
            .collect()
    }

    /// Update cooldowns (call each frame)
    /// This can clean up finished cooldowns to save memory
    pub fn update(&mut self, current_frame: FrameCount) {
        // Mark completed cooldowns as inactive
        for cooldown in self.power_cooldowns.values_mut() {
            if cooldown.active && current_frame >= cooldown.ready_frame {
                cooldown.active = false;
            }
        }

        for cooldown in self.group_cooldowns.values_mut() {
            if cooldown.active && current_frame >= cooldown.ready_frame {
                cooldown.active = false;
            }
        }
    }
}

/// Cooldown reduction modifiers
/// Tracks upgrades and other effects that reduce cooldowns
#[derive(Debug, Clone)]
pub struct CooldownModifiers {
    /// Global cooldown reduction (0.0 = no reduction, 0.5 = 50% reduction, 1.0 = instant)
    pub global_reduction: f32,

    /// Per-power type reductions
    pub power_reductions: HashMap<SpecialPowerType, f32>,

    /// Per-group reductions
    pub group_reductions: HashMap<CooldownGroup, f32>,
}

impl Default for CooldownModifiers {
    fn default() -> Self {
        Self::new()
    }
}

impl CooldownModifiers {
    /// Create new modifiers with no reductions
    pub fn new() -> Self {
        Self {
            global_reduction: 0.0,
            power_reductions: HashMap::new(),
            group_reductions: HashMap::new(),
        }
    }

    /// Set global cooldown reduction (clamped to 0.0-0.9)
    pub fn set_global_reduction(&mut self, reduction: f32) {
        self.global_reduction = reduction.clamp(0.0, 0.9);
    }

    /// Set power-specific cooldown reduction
    pub fn set_power_reduction(&mut self, power_type: SpecialPowerType, reduction: f32) {
        self.power_reductions
            .insert(power_type, reduction.clamp(0.0, 0.9));
    }

    /// Set group-wide cooldown reduction
    pub fn set_group_reduction(&mut self, group: CooldownGroup, reduction: f32) {
        self.group_reductions
            .insert(group, reduction.clamp(0.0, 0.9));
    }

    /// Calculate effective cooldown duration after applying all reductions
    /// Returns modified duration in frames
    pub fn apply_modifiers(
        &self,
        power_type: SpecialPowerType,
        group: CooldownGroup,
        base_duration: FrameCount,
    ) -> FrameCount {
        let mut multiplier = 1.0 - self.global_reduction;

        // Apply power-specific reduction
        if let Some(&power_reduction) = self.power_reductions.get(&power_type) {
            multiplier *= 1.0 - power_reduction;
        }

        // Apply group reduction
        if group != CooldownGroup::None {
            if let Some(&group_reduction) = self.group_reductions.get(&group) {
                multiplier *= 1.0 - group_reduction;
            }
        }

        // Ensure minimum 10% cooldown remains
        multiplier = multiplier.max(0.1);

        (base_duration as f32 * multiplier) as FrameCount
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cooldown_state() {
        let mut state = CooldownState::new_active(0, 100);
        assert!(state.active);
        assert!(!state.is_ready(50));
        assert!(state.is_ready(100));
        assert!(state.is_ready(150));
        assert_eq!(state.remaining_frames(50), 50);
        assert_eq!(state.remaining_frames(100), 0);
    }

    #[test]
    fn test_cooldown_progress() {
        let state = CooldownState::new_active(0, 100);
        assert_eq!(state.get_progress(0), 0.0);
        assert_eq!(state.get_progress(50), 0.5);
        assert_eq!(state.get_progress(100), 1.0);
    }

    #[test]
    fn test_cooldown_manager_individual() {
        let mut manager = CooldownManager::new();
        manager.register_power(SpecialPowerType::A10ThunderboltStrike, CooldownGroup::None);

        // Initially ready
        assert!(manager.is_power_ready(SpecialPowerType::A10ThunderboltStrike, 0));

        // Start cooldown
        manager.start_cooldown(SpecialPowerType::A10ThunderboltStrike, 0, 1000);
        assert!(!manager.is_power_ready(SpecialPowerType::A10ThunderboltStrike, 500));
        assert!(manager.is_power_ready(SpecialPowerType::A10ThunderboltStrike, 1000));
    }

    #[test]
    fn test_cooldown_manager_shared_group() {
        let mut manager = CooldownManager::new();
        manager.register_power(SpecialPowerType::CarpetBomb, CooldownGroup::Airstrike);
        manager.register_power(
            SpecialPowerType::A10ThunderboltStrike,
            CooldownGroup::Airstrike,
        );

        // Both initially ready
        assert!(manager.is_power_ready(SpecialPowerType::CarpetBomb, 0));
        assert!(manager.is_power_ready(SpecialPowerType::A10ThunderboltStrike, 0));

        // Use carpet bomb
        manager.start_cooldown(SpecialPowerType::CarpetBomb, 0, 1000);

        // Both should be on cooldown (shared group)
        assert!(!manager.is_power_ready(SpecialPowerType::CarpetBomb, 500));
        assert!(!manager.is_power_ready(SpecialPowerType::A10ThunderboltStrike, 500));

        // Both ready after cooldown
        assert!(manager.is_power_ready(SpecialPowerType::CarpetBomb, 1000));
        assert!(manager.is_power_ready(SpecialPowerType::A10ThunderboltStrike, 1000));
    }

    #[test]
    fn test_cooldown_modifiers() {
        let mut modifiers = CooldownModifiers::new();

        // No reduction
        assert_eq!(
            modifiers.apply_modifiers(
                SpecialPowerType::A10ThunderboltStrike,
                CooldownGroup::None,
                1000
            ),
            1000
        );

        // 20% global reduction
        modifiers.set_global_reduction(0.2);
        assert_eq!(
            modifiers.apply_modifiers(
                SpecialPowerType::A10ThunderboltStrike,
                CooldownGroup::None,
                1000
            ),
            800
        );

        // Additional 30% power-specific reduction
        modifiers.set_power_reduction(SpecialPowerType::A10ThunderboltStrike, 0.3);
        // Combined: 1000 * 0.8 * 0.7 = 560
        assert_eq!(
            modifiers.apply_modifiers(
                SpecialPowerType::A10ThunderboltStrike,
                CooldownGroup::None,
                1000
            ),
            560
        );
    }

    #[test]
    fn test_cooldown_manager_update() {
        let mut manager = CooldownManager::new();
        manager.register_power(SpecialPowerType::A10ThunderboltStrike, CooldownGroup::None);

        manager.start_cooldown(SpecialPowerType::A10ThunderboltStrike, 0, 100);

        // Before update, cooldown is active
        let state = manager
            .power_cooldowns
            .get(&SpecialPowerType::A10ThunderboltStrike)
            .unwrap();
        assert!(state.active);

        // After update past ready frame, cooldown should be marked inactive
        manager.update(150);
        let state = manager
            .power_cooldowns
            .get(&SpecialPowerType::A10ThunderboltStrike)
            .unwrap();
        assert!(!state.active);
    }

    #[test]
    fn test_reset_cooldowns() {
        let mut manager = CooldownManager::new();
        manager.register_power(
            SpecialPowerType::A10ThunderboltStrike,
            CooldownGroup::Airstrike,
        );

        manager.start_cooldown(SpecialPowerType::A10ThunderboltStrike, 0, 1000);
        assert!(!manager.is_power_ready(SpecialPowerType::A10ThunderboltStrike, 500));

        // Reset power cooldown
        manager.reset_power_cooldown(SpecialPowerType::A10ThunderboltStrike);
        manager.reset_group_cooldown(CooldownGroup::Airstrike);
        assert!(manager.is_power_ready(SpecialPowerType::A10ThunderboltStrike, 500));
    }
}
