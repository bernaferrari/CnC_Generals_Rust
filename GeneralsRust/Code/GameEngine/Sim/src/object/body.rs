//! C++ ActiveBody health thresholds and drawable model conditions.

use super::BodyDamageType;

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
