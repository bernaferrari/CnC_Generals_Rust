//! Host PoisonedBehavior residual (DoT after poison/toxin hit).
//!
//! C++: `PoisonedBehavior::onDamage` starts effects when `DAMAGE_POISON`.
//! Every `PoisonDamageInterval` retakes the original poison amount as
//! UNRESISTABLE (with poison FX override) until `PoisonDuration` after last dose.
//! Healing clears poison. Tint TINT_STATUS_POISONED while active.
//!
//! Residual playability slice:
//! - Interval 100ms → 3 frames @ 30 FPS
//! - Duration 3000ms → 90 frames after last poison hit
//! - Retake damage amount = last poison hit dealt
//! - DeathType POISONED residual
//! - Presentation tint poisoned while active
//!
//! Fail-closed: not full Drawable tint pulse matrix / sleep-frame optimizer /
//! death-type beta/gamma matrix beyond residual death type field.

use crate::game_logic::host_usa_pilot::HostDeathType;
use serde::{Deserialize, Serialize};

pub const POISON_LOGIC_FPS: f32 = 30.0;
/// Retail PoisonDamageInterval residual (msec).
pub const POISON_DAMAGE_INTERVAL_MS: u32 = 100;
/// Retail PoisonDuration residual (msec).
pub const POISON_DURATION_MS: u32 = 3000;

#[inline]
pub fn ms_to_frames(ms: u32) -> u32 {
    ((ms as f32) * POISON_LOGIC_FPS / 1000.0).round().max(1.0) as u32
}

pub fn poison_interval_frames() -> u32 {
    ms_to_frames(POISON_DAMAGE_INTERVAL_MS)
}

pub fn poison_duration_frames() -> u32 {
    ms_to_frames(POISON_DURATION_MS)
}

/// True if damage type should start PoisonedBehavior residual.
pub fn is_poison_damage_type(damage_type: crate::game_logic::combat::DamageType) -> bool {
    use crate::game_logic::combat::DamageType;
    matches!(damage_type, DamageType::Toxin | DamageType::Anthrax)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostPoisonedBehaviorData {
    /// Absolute frame for next DoT tick (0 = inactive).
    pub poison_damage_frame: u32,
    /// Absolute frame when poison ends (0 = inactive).
    pub poison_overall_stop_frame: u32,
    /// Damage amount retaken each interval.
    pub poison_damage_amount: f32,
    /// C++ death type residual while poisoned.
    pub death_type: HostDeathType,
    /// Presentation: TINT_STATUS_POISONED residual.
    pub tint_poisoned: bool,
    /// Cumulative DoT applications this infection.
    pub tick_count: u32,
    /// Cumulative DoT damage applied.
    pub total_dot_damage: f32,
    /// When true, absolute frames are synced on first tick with real `now`.
    pub needs_frame_sync: bool,
}

impl Default for HostPoisonedBehaviorData {
    fn default() -> Self {
        Self {
            poison_damage_frame: 0,
            poison_overall_stop_frame: 0,
            poison_damage_amount: 0.0,
            death_type: HostDeathType::Poisoned,
            tint_poisoned: false,
            tick_count: 0,
            total_dot_damage: 0.0,
            needs_frame_sync: false,
        }
    }
}

impl HostPoisonedBehaviorData {
    pub fn is_active(&self) -> bool {
        self.poison_overall_stop_frame != 0 || self.needs_frame_sync
    }

    /// C++ startPoisonedEffects residual.
    pub fn start_poisoned_effects(
        &mut self,
        now: u32,
        damage_dealt: f32,
        death_type: HostDeathType,
    ) {
        let interval = poison_interval_frames();
        let duration = poison_duration_frames();
        self.poison_damage_amount = damage_dealt.max(0.0);
        self.death_type = death_type;
        self.tint_poisoned = true;
        if now == 0 {
            // Defer absolute frame anchors until GameLogic tick provides `now`.
            self.needs_frame_sync = true;
            if self.poison_overall_stop_frame == 0 {
                self.poison_damage_frame = 0;
                self.poison_overall_stop_frame = 0;
            }
            // Keep active marker via tint + amount; frames filled on sync.
            self.poison_overall_stop_frame = self.poison_overall_stop_frame.max(1); // active sentinel
            return;
        }
        self.needs_frame_sync = false;
        self.poison_overall_stop_frame = now.saturating_add(duration);
        if self.poison_damage_frame != 0 {
            // Re-poison: don't push damage counter later than now+interval.
            self.poison_damage_frame = self.poison_damage_frame.min(now.saturating_add(interval));
        } else {
            self.poison_damage_frame = now.saturating_add(interval);
        }
    }

    fn sync_frames_if_needed(&mut self, now: u32) {
        if !self.needs_frame_sync || now == 0 {
            return;
        }
        let interval = poison_interval_frames();
        let duration = poison_duration_frames();
        self.poison_overall_stop_frame = now.saturating_add(duration);
        self.poison_damage_frame = now.saturating_add(interval);
        self.needs_frame_sync = false;
    }

    /// C++ stopPoisonedEffects residual.
    pub fn stop_poisoned_effects(&mut self) {
        self.poison_damage_frame = 0;
        self.poison_overall_stop_frame = 0;
        self.poison_damage_amount = 0.0;
        self.tint_poisoned = false;
        self.needs_frame_sync = false;
    }

    /// C++ update residual. Returns Some(damage) when a DoT tick fires.
    pub fn tick(&mut self, now: u32) -> Option<(f32, HostDeathType)> {
        if self.poison_overall_stop_frame == 0 && !self.needs_frame_sync {
            return None;
        }
        self.sync_frames_if_needed(now);
        if self.poison_overall_stop_frame == 0 {
            return None;
        }
        let mut dmg = None;
        if self.poison_damage_frame != 0 && now >= self.poison_damage_frame {
            let amount = self.poison_damage_amount;
            if amount > 0.0 {
                dmg = Some((amount, self.death_type));
                self.tick_count = self.tick_count.saturating_add(1);
                self.total_dot_damage += amount;
            }
            self.poison_damage_frame = now.saturating_add(poison_interval_frames());
        }
        if now >= self.poison_overall_stop_frame {
            // Stop only if not effectively dead — caller decides; we clear when alive path.
            // Caller should call stop if object still "alive enough".
            // We clear stop frames here only after reporting last tick.
            if dmg.is_none() {
                // duration elapsed between ticks
            }
        }
        dmg
    }

    /// True if duration elapsed and effects should clear (when not dead).
    pub fn should_stop(&self, now: u32) -> bool {
        self.poison_overall_stop_frame != 0 && now >= self.poison_overall_stop_frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poison_ticks_then_stops() {
        let mut p = HostPoisonedBehaviorData::default();
        p.start_poisoned_effects(0, 10.0, HostDeathType::Poisoned);
        assert!(p.is_active());
        assert!(p.tint_poisoned);
        let interval = poison_interval_frames();
        let mut ticks = 0;
        for f in 0..200 {
            if let Some((d, _)) = p.tick(f) {
                assert!((d - 10.0).abs() < 0.01);
                ticks += 1;
            }
            if p.should_stop(f) && f > interval {
                p.stop_poisoned_effects();
                break;
            }
        }
        assert!(ticks >= 1);
        assert!(!p.is_active());
    }

    #[test]
    fn repoison_extends_duration() {
        let mut p = HostPoisonedBehaviorData::default();
        p.start_poisoned_effects(0, 5.0, HostDeathType::Poisoned);
        let stop1 = p.poison_overall_stop_frame;
        p.start_poisoned_effects(20, 8.0, HostDeathType::Poisoned);
        assert!(p.poison_overall_stop_frame > stop1);
        assert!((p.poison_damage_amount - 8.0).abs() < 0.01);
    }
}
