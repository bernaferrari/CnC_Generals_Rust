//! C++ ActiveBody health thresholds and drawable model conditions.

use super::BodyDamageType;
use game_engine::common::ini::ini_game_data::BodyDamageRules;
use serde::{Deserialize, Serialize};

/// ActiveBody::calcDamageState compares the IEEE ratio directly.
pub fn classify_body_damage(
    health: f32,
    maximum: f32,
    (damaged, really_damaged): (f32, f32),
) -> BodyDamageType {
    let ratio = health / maximum;
    if ratio > damaged {
        BodyDamageType::Pristine
    } else if ratio > really_damaged {
        BodyDamageType::Damaged
    } else if ratio > 0.0 {
        BodyDamageType::ReallyDamaged
    } else {
        BodyDamageType::Rubble
    }
}

/// ActiveBody::setDamageState subtracts 1 HP to cross the strict threshold.
pub fn health_for_body_damage_state(
    maximum: f32,
    state: BodyDamageType,
    (damaged, really_damaged): (f32, f32),
) -> f32 {
    let ratio = match state {
        BodyDamageType::Pristine => 1.0,
        BodyDamageType::Damaged => damaged,
        BodyDamageType::ReallyDamaged => really_damaged,
        BodyDamageType::Rubble => 0.0,
    };
    (maximum * ratio - 1.0).max(0.0)
}

/// Damage model-condition indices in the retail BitFlags table.
pub const MC_BIT_DAMAGED: u32 = 3;
pub const MC_BIT_REALLYDAMAGED: u32 = 4;
pub const MC_BIT_RUBBLE: u32 = 5;

/// Clear DAMAGED/REALLYDAMAGED/RUBBLE and set the bit for `state` (C++ reactToBodyDamageStateChange).
pub fn apply_body_damage_model_bits(bits: u128, state: BodyDamageType) -> u128 {
    let mut b = bits;
    b &= !(1u128 << MC_BIT_DAMAGED);
    b &= !(1u128 << MC_BIT_REALLYDAMAGED);
    b &= !(1u128 << MC_BIT_RUBBLE);
    match state {
        BodyDamageType::Pristine => {}
        BodyDamageType::Damaged => b |= 1u128 << MC_BIT_DAMAGED,
        BodyDamageType::ReallyDamaged => b |= 1u128 << MC_BIT_REALLYDAMAGED,
        BodyDamageType::Rubble => b |= 1u128 << MC_BIT_RUBBLE,
    }
    b
}

/// Current and maximum hit points of one body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub current: f32,
    pub maximum: f32,
}

impl Health {
    pub fn new(max_health: f32) -> Self {
        Self {
            current: max_health,
            maximum: max_health,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.current > 0.0
    }

    pub fn is_full(&self) -> bool {
        self.current >= self.maximum
    }

    pub fn percentage(&self) -> f32 {
        if self.maximum > 0.0 {
            self.current / self.maximum
        } else {
            0.0
        }
    }

    pub fn damage(&mut self, amount: f32) {
        self.current = (self.current - amount).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.maximum);
    }

    /// Apply ActiveBody::internalChangeHealth and return the prior hit points.
    pub fn apply_delta(&mut self, delta: f32, immortal: bool) -> f32 {
        let previous = self.current;
        let delta = if immortal {
            delta.max(-previous + 1.0)
        } else {
            delta
        };
        self.current += delta;
        if self.current > self.maximum {
            self.current = self.maximum;
        }
        if self.current < 0.0 {
            self.current = 0.0;
        }
        previous
    }

    /// Set the maximum and compute the first delta for ActiveBody::setMaxHealth.
    /// Callers apply it through the normal health transition, then reread
    /// current HP before applying any final cap.
    pub fn set_maximum(&mut self, maximum: f32, mode: MaxHealthChange) -> Option<f32> {
        let old_maximum = self.maximum;
        self.maximum = maximum;
        match mode {
            MaxHealthChange::PreserveRatio => {
                Some(maximum * (self.current / old_maximum) - self.current)
            }
            MaxHealthChange::AddToCurrent => Some(maximum - old_maximum),
            MaxHealthChange::KeepCurrent => None,
            MaxHealthChange::FullyHeal => Some(maximum - self.current),
        }
    }
}

/// BodyModuleInterface::MaxHealthChangeType.
#[derive(Clone, Copy)]
pub enum MaxHealthChange {
    PreserveRatio,
    AddToCurrent,
    KeepCurrent,
    FullyHeal,
}

/// Standalone compatibility cannot masquerade as admitted application content.
/// This is immutable definition data, not a second writable Body state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BodyDamagePolicy {
    #[default]
    StandaloneRetail,
    Receiving(BodyDamageRules),
}
impl BodyDamagePolicy {
    pub fn health_for_state(self, maximum: f32, state: BodyDamageType) -> f32 {
        let rules = match self {
            Self::StandaloneRetail => BodyDamageRules::standalone_retail(),
            Self::Receiving(rules) => rules,
        };
        health_for_body_damage_state(maximum, state, rules.thresholds())
    }
    pub fn classify(self, health: f32, maximum: f32) -> BodyDamageType {
        let rules = match self {
            Self::StandaloneRetail => BodyDamageRules::standalone_retail(),
            Self::Receiving(rules) => rules,
        };
        classify_body_damage(health, maximum, rules.thresholds())
    }
}
