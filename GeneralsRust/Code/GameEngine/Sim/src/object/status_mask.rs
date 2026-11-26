//! Simulation object status mask in the original bit order.

use super::ObjectStatusTypes;
use bitflags::bitflags;
use serde::{Deserialize, Serialize};

// Mask types for various object properties
bitflags! {
    /// Object status mask (matching C++ ObjectStatusMaskType)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ObjectStatusMaskType: u64 {
        const NONE = 0;
        const DESTROYED = 1u64 << ObjectStatusTypes::Destroyed as u32;
        const CAN_ATTACK = 1u64 << ObjectStatusTypes::CanAttack as u32;
        const UNDER_CONSTRUCTION = 1u64 << ObjectStatusTypes::UnderConstruction as u32;
        const UNSELECTABLE = 1u64 << ObjectStatusTypes::Unselectable as u32;
        const NO_COLLISIONS = 1u64 << ObjectStatusTypes::NoCollisions as u32;
        const NO_ATTACK = 1u64 << ObjectStatusTypes::NoAttack as u32;
        const AIRBORNE_TARGET = 1u64 << ObjectStatusTypes::AirborneTarget as u32;
        const PARACHUTING = 1u64 << ObjectStatusTypes::Parachuting as u32;
        const REPULSOR = 1u64 << ObjectStatusTypes::Repulsor as u32;
        const HIJACKED = 1u64 << ObjectStatusTypes::Hijacked as u32;
        const AFLAME = 1u64 << ObjectStatusTypes::Aflame as u32;
        const BURNED = 1u64 << ObjectStatusTypes::Burned as u32;
        const WET = 1u64 << ObjectStatusTypes::Wet as u32;
        const IS_FIRING_WEAPON = 1u64 << ObjectStatusTypes::IsFiringWeapon as u32;
        const BRAKING = 1u64 << ObjectStatusTypes::Braking as u32;
        const STEALTHED = 1u64 << ObjectStatusTypes::Stealthed as u32;
        const DETECTED = 1u64 << ObjectStatusTypes::Detected as u32;
        const CAN_STEALTH = 1u64 << ObjectStatusTypes::CanStealth as u32;
        const SOLD = 1u64 << ObjectStatusTypes::Sold as u32;
        const UNDERGOING_REPAIR = 1u64 << ObjectStatusTypes::UndergoingRepair as u32;
        const RECONSTRUCTING = 1u64 << ObjectStatusTypes::Reconstructing as u32;
        const MASKED = 1u64 << ObjectStatusTypes::Masked as u32;
        const IS_ATTACKING = 1u64 << ObjectStatusTypes::IsAttacking as u32;
        const IS_USING_ABILITY = 1u64 << ObjectStatusTypes::IsUsingAbility as u32;
        const IS_AIMING_WEAPON = 1u64 << ObjectStatusTypes::IsAimingWeapon as u32;
        const NO_ATTACK_FROM_AI = 1u64 << ObjectStatusTypes::NoAttackFromAi as u32;
        const IGNORING_STEALTH = 1u64 << ObjectStatusTypes::IgnoringStealth as u32;
        const IS_CAR_BOMB = 1u64 << ObjectStatusTypes::IsCarBomb as u32;
        const DECK_HEIGHT_OFFSET = 1u64 << ObjectStatusTypes::DeckHeightOffset as u32;
        const RIDER1 = 1u64 << ObjectStatusTypes::Rider1 as u32;
        const RIDER2 = 1u64 << ObjectStatusTypes::Rider2 as u32;
        const RIDER3 = 1u64 << ObjectStatusTypes::Rider3 as u32;
        const RIDER4 = 1u64 << ObjectStatusTypes::Rider4 as u32;
        const RIDER5 = 1u64 << ObjectStatusTypes::Rider5 as u32;
        const RIDER6 = 1u64 << ObjectStatusTypes::Rider6 as u32;
        const RIDER7 = 1u64 << ObjectStatusTypes::Rider7 as u32;
        const RIDER8 = 1u64 << ObjectStatusTypes::Rider8 as u32;
        const FAERIE_FIRE = 1u64 << ObjectStatusTypes::FaerieFire as u32;
        const MISSILE_KILLING_SELF = 1u64 << ObjectStatusTypes::MissileKillingSelf as u32;
        const REASSIGN_PARKING = 1u64 << ObjectStatusTypes::ReassignParking as u32;
        const BOOBY_TRAPPED = 1u64 << ObjectStatusTypes::BoobyTrapped as u32;
        const IMMOBILE = 1u64 << ObjectStatusTypes::Immobile as u32;
        const DISGUISED = 1u64 << ObjectStatusTypes::Disguised as u32;
        const DEPLOYED = 1u64 << ObjectStatusTypes::Deployed as u32;
    }
}

impl ObjectStatusMaskType {
    /// Empty mask (matches C++ `OBJECT_STATUS_MASK_NONE`)
    pub fn none() -> Self {
        Self::NONE
    }

    /// Create a mask from a single status bit.
    pub const fn from_status(status: ObjectStatusTypes) -> Self {
        match status {
            ObjectStatusTypes::None => Self::NONE,
            _ => Self::from_bits_retain(1u64 << (status as u32)),
        }
    }

    /// Check whether a particular status bit is set.
    pub fn test(&self, status: ObjectStatusTypes) -> bool {
        match status {
            ObjectStatusTypes::None => self.is_empty(),
            _ => self.contains(Self::from_status(status)),
        }
    }

    /// Alias for test() - check whether a particular status bit is set.
    pub fn test_status(&self, status: ObjectStatusTypes) -> bool {
        self.test(status)
    }

    /// Returns true if any status bits are set (mask is not empty).
    pub fn any(&self) -> bool {
        !self.is_empty()
    }

    /// Set a single status bit.
    pub fn set_status(&mut self, status: ObjectStatusTypes) {
        *self |= Self::from_status(status);
    }

    /// Clear a single status bit.
    pub fn clear_status(&mut self, status: ObjectStatusTypes) {
        *self &= !Self::from_status(status);
    }

    /// Parse a list of object-status tokens into a mask, mirroring the legacy helper.
    pub fn parse_tokens<'a, I>(tokens: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let tokens: Vec<&'a str> = tokens.into_iter().collect();
        let has_none = tokens
            .iter()
            .any(|token| token.eq_ignore_ascii_case("NONE"));
        if has_none && tokens.len() > 1 {
            return Err("mixing NONE with other tokens is invalid".to_string());
        }

        generals_game_domain::object_status::parse_object_status_bits(tokens)
            .map(Self::from_bits_retain)
    }

    pub fn from_case_insensitive_name(name: &str) -> Option<Self> {
        Self::parse_tokens(std::iter::once(name)).ok()
    }
}

/// Implement From trait to convert ObjectStatusTypes to ObjectStatusMaskType
impl From<ObjectStatusTypes> for ObjectStatusMaskType {
    fn from(status: ObjectStatusTypes) -> Self {
        Self::from_status(status)
    }
}
