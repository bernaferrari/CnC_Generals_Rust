////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: object_status_types.rs ///////////////////////////////////////////////
// Author: Kris, May 2003
// Desc:   Object status types that are stackable using the BitSet system. Used to be ObjectStatusBits
///////////////////////////////////////////////////////////////////////////////

use bitflags::bitflags;
use lazy_static::lazy_static;

/// Object status types - these are saved, do not insert or remove any!
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectStatusTypes {
    /// No status bit
    None = 0,
    /// Has been destroyed, pending delete
    Destroyed = 1,
    /// Used by garrisoned buildings, is OR'ed with KINDOF_CAN_ATTACK in isAbleToAttack()
    CanAttack = 2,
    /// Object is being constructed and is not yet complete
    UnderConstruction = 3,
    /// This is a negative condition since these statuses are overrides
    Unselectable = 4,
    /// Object should be ignored for object-object collisions (but not object-ground)
    NoCollisions = 5,
    /// Absolute override to being able to attack
    NoAttack = 6,
    /// InTheAir as far as AntiAir weapons are concerned only
    AirborneTarget = 7,
    /// Object is on a parachute
    Parachuting = 8,
    /// Object repulses "KINDOF_CAN_BE_REPULSED" objects
    Repulsor = 9,
    /// Unit is in the possession of an enemy criminal, call the authorities
    Hijacked = 10,
    /// This object is on fire
    Aflame = 11,
    /// This object has already burned as much as it can
    Burned = 12,
    /// Object has been soaked with water
    Wet = 13,
    /// Object is firing a weapon, now. Not true for special attacks
    IsFiringWeapon = 14,
    /// Object is braking, and subverts the physics
    IsBraking = 15,
    /// Object is currently "stealthed"
    Stealthed = 16,
    /// Object is in range of a stealth-detector unit (meaningless if STEALTHED not set)
    Detected = 17,
    /// Object has ability to stealth allowing the stealth update module to run
    CanStealth = 18,
    /// Object is being sold
    Sold = 19,
    /// Object is awaiting/undergoing a repair order that has been issued
    UndergoingRepair = 20,
    /// Reconstructing
    Reconstructing = 21,
    /// Masked objects are not selectable and targetable by players or AI
    Masked = 22,
    /// Object is in the general Attack state (incl. aim, approach, etc.)
    IsAttacking = 23,
    /// Object is in the process of preparing or firing a special ability
    UsingAbility = 24,
    /// Object is aiming a weapon, now. Not true for special attacks
    IsAimingWeapon = 25,
    /// Attacking this object may not be done from commandSource == CMD_FROM_AI
    NoAttackFromAI = 26,
    /// Temporarily ignoring all stealth bits (used only for some special-case mine clearing stuff)
    IgnoringStealth = 27,
    /// Object is now a carbomb
    IsCarBomb = 28,
    /// Object factors deck height on top of ground altitude
    DeckHeightOffset = 29,
    /// Rider status bits
    StatusRider1 = 30,
    StatusRider2 = 31,
    StatusRider3 = 32,
    StatusRider4 = 33,
    StatusRider5 = 34,
    StatusRider6 = 35,
    StatusRider7 = 36,
    StatusRider8 = 37,
    /// Anyone shooting at you shoots faster than normal
    FaerieFire = 38,
    /// Object (likely a missile or bomb) is killing itself
    KillingSelf = 39,
    /// Jet is trying to get a better parking assignment
    ReassignParking = 40,
    /// We need to know we have a booby trap on us so we can detonate it from many different code segments
    BoobyTrapped = 41,
    /// Do not move!
    Immobile = 42,
    /// Object is disguised (a type of stealth)
    Disguised = 43,
    /// Object is deployed
    Deployed = 44,
}

bitflags! {
    // Object status mask type using bitflags
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ObjectStatusMaskType: u64 {
        const NONE                  = 1 << 0;
        const DESTROYED             = 1 << 1;
        const CAN_ATTACK            = 1 << 2;
        const UNDER_CONSTRUCTION    = 1 << 3;
        const UNSELECTABLE          = 1 << 4;
        const NO_COLLISIONS         = 1 << 5;
        const NO_ATTACK             = 1 << 6;
        const AIRBORNE_TARGET       = 1 << 7;
        const PARACHUTING           = 1 << 8;
        const REPULSOR              = 1 << 9;
        const HIJACKED              = 1 << 10;
        const AFLAME                = 1 << 11;
        const BURNED                = 1 << 12;
        const WET                   = 1 << 13;
        const IS_FIRING_WEAPON      = 1 << 14;
        const IS_BRAKING            = 1 << 15;
        const STEALTHED             = 1 << 16;
        const DETECTED              = 1 << 17;
        const CAN_STEALTH           = 1 << 18;
        const SOLD                  = 1 << 19;
        const UNDERGOING_REPAIR     = 1 << 20;
        const RECONSTRUCTING        = 1 << 21;
        const MASKED                = 1 << 22;
        const IS_ATTACKING          = 1 << 23;
        const USING_ABILITY         = 1 << 24;
        const IS_AIMING_WEAPON      = 1 << 25;
        const NO_ATTACK_FROM_AI     = 1 << 26;
        const IGNORING_STEALTH      = 1 << 27;
        const IS_CARBOMB            = 1 << 28;
        const DECK_HEIGHT_OFFSET    = 1 << 29;
        const STATUS_RIDER1         = 1 << 30;
        const STATUS_RIDER2         = 1 << 31;
        const STATUS_RIDER3         = 1 << 32;
        const STATUS_RIDER4         = 1 << 33;
        const STATUS_RIDER5         = 1 << 34;
        const STATUS_RIDER6         = 1 << 35;
        const STATUS_RIDER7         = 1 << 36;
        const STATUS_RIDER8         = 1 << 37;
        const FAERIE_FIRE           = 1 << 38;
        const KILLING_SELF          = 1 << 39;
        const REASSIGN_PARKING      = 1 << 40;
        const BOOBY_TRAPPED         = 1 << 41;
        const IMMOBILE              = 1 << 42;
        const DISGUISED             = 1 << 43;
        const DEPLOYED              = 1 << 44;
    }
}

impl ObjectStatusMaskType {
    /// Resolve a mask bit by its canonical (case-insensitive) name.
    pub fn from_case_insensitive_name(name: &str) -> Option<Self> {
        STATUS_FLAG_LOOKUP
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, flag)| *flag)
    }

    /// Parse a sequence of status-bit tokens into a mask.
    ///
    /// Mirrors the legacy BitFlags::parse semantics:
    /// - `NONE` clears the mask and cannot be combined with other tokens.
    /// - `+FLAG` and `-FLAG` incrementally set or clear bits; these cannot be
    ///   mixed with bare flag names.
    /// - Bare flag names replace the mask (first token clears, subsequent add).
    pub fn parse_tokens<'a, I>(tokens: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut mask = Self::empty();
        let mut found_normal = false;
        let mut found_add_or_sub = false;

        for raw in tokens {
            let token = raw.trim();
            if token.is_empty() {
                continue;
            }

            let token = token.trim_matches(',');

            if token.is_empty() {
                continue;
            }

            if token.eq_ignore_ascii_case("NONE") {
                if found_normal || found_add_or_sub {
                    return Err("cannot mix NONE with other object status tokens".to_string());
                }
                mask = Self::empty();
                return Ok(mask);
            }

            let (op, name) = match token.chars().next() {
                Some('+') | Some('-') => (token.chars().next(), token[1..].trim()),
                _ => (None, token),
            };

            if name.is_empty() {
                return Err(format!("invalid object status token '{token}'"));
            }

            let flag = Self::from_case_insensitive_name(name)
                .ok_or_else(|| format!("unknown object status '{name}'"))?;

            match op {
                Some('+') => {
                    if found_normal {
                        return Err(
                            "cannot mix additive object status tokens with direct listings"
                                .to_string(),
                        );
                    }
                    mask.insert(flag);
                    found_add_or_sub = true;
                }
                Some('-') => {
                    if found_normal {
                        return Err(
                            "cannot mix subtractive object status tokens with direct listings"
                                .to_string(),
                        );
                    }
                    mask.remove(flag);
                    found_add_or_sub = true;
                }
                None => {
                    if found_add_or_sub {
                        return Err(
                            "cannot mix direct object status listings with +/- modifiers"
                                .to_string(),
                        );
                    }
                    if !found_normal {
                        mask = Self::empty();
                    }
                    mask.insert(flag);
                    found_normal = true;
                }
                Some(other) => {
                    return Err(format!(
                        "unsupported object status modifier '{}{}'",
                        other, name
                    ));
                }
            }
        }

        Ok(mask)
    }

    /// Create an object status mask from a single status type
    pub fn from_status(status: ObjectStatusTypes) -> Self {
        match status {
            ObjectStatusTypes::None => Self::NONE,
            ObjectStatusTypes::Destroyed => Self::DESTROYED,
            ObjectStatusTypes::CanAttack => Self::CAN_ATTACK,
            ObjectStatusTypes::UnderConstruction => Self::UNDER_CONSTRUCTION,
            ObjectStatusTypes::Unselectable => Self::UNSELECTABLE,
            ObjectStatusTypes::NoCollisions => Self::NO_COLLISIONS,
            ObjectStatusTypes::NoAttack => Self::NO_ATTACK,
            ObjectStatusTypes::AirborneTarget => Self::AIRBORNE_TARGET,
            ObjectStatusTypes::Parachuting => Self::PARACHUTING,
            ObjectStatusTypes::Repulsor => Self::REPULSOR,
            ObjectStatusTypes::Hijacked => Self::HIJACKED,
            ObjectStatusTypes::Aflame => Self::AFLAME,
            ObjectStatusTypes::Burned => Self::BURNED,
            ObjectStatusTypes::Wet => Self::WET,
            ObjectStatusTypes::IsFiringWeapon => Self::IS_FIRING_WEAPON,
            ObjectStatusTypes::IsBraking => Self::IS_BRAKING,
            ObjectStatusTypes::Stealthed => Self::STEALTHED,
            ObjectStatusTypes::Detected => Self::DETECTED,
            ObjectStatusTypes::CanStealth => Self::CAN_STEALTH,
            ObjectStatusTypes::Sold => Self::SOLD,
            ObjectStatusTypes::UndergoingRepair => Self::UNDERGOING_REPAIR,
            ObjectStatusTypes::Reconstructing => Self::RECONSTRUCTING,
            ObjectStatusTypes::Masked => Self::MASKED,
            ObjectStatusTypes::IsAttacking => Self::IS_ATTACKING,
            ObjectStatusTypes::UsingAbility => Self::USING_ABILITY,
            ObjectStatusTypes::IsAimingWeapon => Self::IS_AIMING_WEAPON,
            ObjectStatusTypes::NoAttackFromAI => Self::NO_ATTACK_FROM_AI,
            ObjectStatusTypes::IgnoringStealth => Self::IGNORING_STEALTH,
            ObjectStatusTypes::IsCarBomb => Self::IS_CARBOMB,
            ObjectStatusTypes::DeckHeightOffset => Self::DECK_HEIGHT_OFFSET,
            ObjectStatusTypes::StatusRider1 => Self::STATUS_RIDER1,
            ObjectStatusTypes::StatusRider2 => Self::STATUS_RIDER2,
            ObjectStatusTypes::StatusRider3 => Self::STATUS_RIDER3,
            ObjectStatusTypes::StatusRider4 => Self::STATUS_RIDER4,
            ObjectStatusTypes::StatusRider5 => Self::STATUS_RIDER5,
            ObjectStatusTypes::StatusRider6 => Self::STATUS_RIDER6,
            ObjectStatusTypes::StatusRider7 => Self::STATUS_RIDER7,
            ObjectStatusTypes::StatusRider8 => Self::STATUS_RIDER8,
            ObjectStatusTypes::FaerieFire => Self::FAERIE_FIRE,
            ObjectStatusTypes::KillingSelf => Self::KILLING_SELF,
            ObjectStatusTypes::ReassignParking => Self::REASSIGN_PARKING,
            ObjectStatusTypes::BoobyTrapped => Self::BOOBY_TRAPPED,
            ObjectStatusTypes::Immobile => Self::IMMOBILE,
            ObjectStatusTypes::Disguised => Self::DISGUISED,
            ObjectStatusTypes::Deployed => Self::DEPLOYED,
        }
    }

    /// Test if a specific status is set
    pub fn test_status(&self, status: ObjectStatusTypes) -> bool {
        self.contains(Self::from_status(status))
    }

    /// Test if any of the flags in the given mask are set
    pub fn test_any(&self, mask: ObjectStatusMaskType) -> bool {
        self.intersects(mask)
    }

    /// Test set and clear flags
    pub fn test_set_and_clear(
        &self,
        must_be_set: ObjectStatusMaskType,
        must_be_clear: ObjectStatusMaskType,
    ) -> bool {
        self.contains(must_be_set) && !self.intersects(must_be_clear)
    }

    /// Clear all bits
    pub fn clear_all(&mut self) {
        *self = ObjectStatusMaskType::empty();
    }

    /// Set all bits
    pub fn set_all(&mut self) {
        *self = ObjectStatusMaskType::all();
    }

    /// Flip all bits
    pub fn flip_all(&mut self) {
        *self = self.complement();
    }
}

/// String names for object status bits (for debugging/serialization)
pub const OBJECT_STATUS_BIT_NAMES: &[&str] = &[
    "NONE",
    "DESTROYED",
    "CAN_ATTACK",
    "UNDER_CONSTRUCTION",
    "UNSELECTABLE",
    "NO_COLLISIONS",
    "NO_ATTACK",
    "AIRBORNE_TARGET",
    "PARACHUTING",
    "REPULSOR",
    "HIJACKED",
    "AFLAME",
    "BURNED",
    "WET",
    "IS_FIRING_WEAPON",
    "IS_BRAKING",
    "STEALTHED",
    "DETECTED",
    "CAN_STEALTH",
    "SOLD",
    "UNDERGOING_REPAIR",
    "RECONSTRUCTING",
    "MASKED",
    "IS_ATTACKING",
    "USING_ABILITY",
    "IS_AIMING_WEAPON",
    "NO_ATTACK_FROM_AI",
    "IGNORING_STEALTH",
    "IS_CARBOMB",
    "DECK_HEIGHT_OFFSET",
    "STATUS_RIDER1",
    "STATUS_RIDER2",
    "STATUS_RIDER3",
    "STATUS_RIDER4",
    "STATUS_RIDER5",
    "STATUS_RIDER6",
    "STATUS_RIDER7",
    "STATUS_RIDER8",
    "FAERIE_FIRE",
    "KILLING_SELF",
    "REASSIGN_PARKING",
    "BOOBY_TRAPPED",
    "IMMOBILE",
    "DISGUISED",
    "DEPLOYED",
];

const STATUS_FLAG_LOOKUP: &[(&str, ObjectStatusMaskType)] = &[
    ("DESTROYED", ObjectStatusMaskType::DESTROYED),
    ("CAN_ATTACK", ObjectStatusMaskType::CAN_ATTACK),
    (
        "UNDER_CONSTRUCTION",
        ObjectStatusMaskType::UNDER_CONSTRUCTION,
    ),
    ("UNSELECTABLE", ObjectStatusMaskType::UNSELECTABLE),
    ("NO_COLLISIONS", ObjectStatusMaskType::NO_COLLISIONS),
    ("NO_ATTACK", ObjectStatusMaskType::NO_ATTACK),
    ("AIRBORNE_TARGET", ObjectStatusMaskType::AIRBORNE_TARGET),
    ("PARACHUTING", ObjectStatusMaskType::PARACHUTING),
    ("REPULSOR", ObjectStatusMaskType::REPULSOR),
    ("HIJACKED", ObjectStatusMaskType::HIJACKED),
    ("AFLAME", ObjectStatusMaskType::AFLAME),
    ("BURNED", ObjectStatusMaskType::BURNED),
    ("WET", ObjectStatusMaskType::WET),
    ("IS_FIRING_WEAPON", ObjectStatusMaskType::IS_FIRING_WEAPON),
    ("IS_BRAKING", ObjectStatusMaskType::IS_BRAKING),
    ("STEALTHED", ObjectStatusMaskType::STEALTHED),
    ("DETECTED", ObjectStatusMaskType::DETECTED),
    ("CAN_STEALTH", ObjectStatusMaskType::CAN_STEALTH),
    ("SOLD", ObjectStatusMaskType::SOLD),
    ("UNDERGOING_REPAIR", ObjectStatusMaskType::UNDERGOING_REPAIR),
    ("RECONSTRUCTING", ObjectStatusMaskType::RECONSTRUCTING),
    ("MASKED", ObjectStatusMaskType::MASKED),
    ("IS_ATTACKING", ObjectStatusMaskType::IS_ATTACKING),
    ("USING_ABILITY", ObjectStatusMaskType::USING_ABILITY),
    ("IS_AIMING_WEAPON", ObjectStatusMaskType::IS_AIMING_WEAPON),
    ("NO_ATTACK_FROM_AI", ObjectStatusMaskType::NO_ATTACK_FROM_AI),
    ("IGNORING_STEALTH", ObjectStatusMaskType::IGNORING_STEALTH),
    ("IS_CARBOMB", ObjectStatusMaskType::IS_CARBOMB),
    (
        "DECK_HEIGHT_OFFSET",
        ObjectStatusMaskType::DECK_HEIGHT_OFFSET,
    ),
    ("STATUS_RIDER1", ObjectStatusMaskType::STATUS_RIDER1),
    ("STATUS_RIDER2", ObjectStatusMaskType::STATUS_RIDER2),
    ("STATUS_RIDER3", ObjectStatusMaskType::STATUS_RIDER3),
    ("STATUS_RIDER4", ObjectStatusMaskType::STATUS_RIDER4),
    ("STATUS_RIDER5", ObjectStatusMaskType::STATUS_RIDER5),
    ("STATUS_RIDER6", ObjectStatusMaskType::STATUS_RIDER6),
    ("STATUS_RIDER7", ObjectStatusMaskType::STATUS_RIDER7),
    ("STATUS_RIDER8", ObjectStatusMaskType::STATUS_RIDER8),
    ("FAERIE_FIRE", ObjectStatusMaskType::FAERIE_FIRE),
    ("KILLING_SELF", ObjectStatusMaskType::KILLING_SELF),
    ("REASSIGN_PARKING", ObjectStatusMaskType::REASSIGN_PARKING),
    ("BOOBY_TRAPPED", ObjectStatusMaskType::BOOBY_TRAPPED),
    ("IMMOBILE", ObjectStatusMaskType::IMMOBILE),
    ("DISGUISED", ObjectStatusMaskType::DISGUISED),
    ("DEPLOYED", ObjectStatusMaskType::DEPLOYED),
];

lazy_static! {
    /// Global object status mask initialized to all zeros
    pub static ref OBJECT_STATUS_MASK_NONE: ObjectStatusMaskType = ObjectStatusMaskType::empty();
}

/// Helper macros for creating object status masks
/// Create a mask with one status
pub fn make_object_status_mask(status: ObjectStatusTypes) -> ObjectStatusMaskType {
    ObjectStatusMaskType::from_status(status)
}

/// Create a mask with two statuses
pub fn make_object_status_mask2(
    status1: ObjectStatusTypes,
    status2: ObjectStatusTypes,
) -> ObjectStatusMaskType {
    ObjectStatusMaskType::from_status(status1) | ObjectStatusMaskType::from_status(status2)
}

/// Create a mask with three statuses
pub fn make_object_status_mask3(
    status1: ObjectStatusTypes,
    status2: ObjectStatusTypes,
    status3: ObjectStatusTypes,
) -> ObjectStatusMaskType {
    ObjectStatusMaskType::from_status(status1)
        | ObjectStatusMaskType::from_status(status2)
        | ObjectStatusMaskType::from_status(status3)
}

/// Create a mask with four statuses
pub fn make_object_status_mask4(
    status1: ObjectStatusTypes,
    status2: ObjectStatusTypes,
    status3: ObjectStatusTypes,
    status4: ObjectStatusTypes,
) -> ObjectStatusMaskType {
    ObjectStatusMaskType::from_status(status1)
        | ObjectStatusMaskType::from_status(status2)
        | ObjectStatusMaskType::from_status(status3)
        | ObjectStatusMaskType::from_status(status4)
}

/// Create a mask with five statuses
pub fn make_object_status_mask5(
    status1: ObjectStatusTypes,
    status2: ObjectStatusTypes,
    status3: ObjectStatusTypes,
    status4: ObjectStatusTypes,
    status5: ObjectStatusTypes,
) -> ObjectStatusMaskType {
    ObjectStatusMaskType::from_status(status1)
        | ObjectStatusMaskType::from_status(status2)
        | ObjectStatusMaskType::from_status(status3)
        | ObjectStatusMaskType::from_status(status4)
        | ObjectStatusMaskType::from_status(status5)
}

/// Test if a specific status is set in the mask
pub fn test_object_status_mask(mask: &ObjectStatusMaskType, status: ObjectStatusTypes) -> bool {
    mask.test_status(status)
}

/// Test if any of the flags in the given mask are set
pub fn test_object_status_mask_any(
    mask: &ObjectStatusMaskType,
    test_mask: &ObjectStatusMaskType,
) -> bool {
    mask.test_any(*test_mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_status_from_name_case_insensitive() {
        assert_eq!(
            ObjectStatusMaskType::from_case_insensitive_name("stealthed"),
            Some(ObjectStatusMaskType::STEALTHED)
        );
        assert_eq!(
            ObjectStatusMaskType::from_case_insensitive_name("Reassign_Parking"),
            Some(ObjectStatusMaskType::REASSIGN_PARKING)
        );
        assert!(ObjectStatusMaskType::from_case_insensitive_name("not_a_status").is_none());
    }

    #[test]
    fn object_status_parse_tokens_direct() {
        let mask = ObjectStatusMaskType::parse_tokens(["STEALTHED", "DETECTED"].iter().copied())
            .expect("parse succeeds");
        assert!(mask.contains(ObjectStatusMaskType::STEALTHED));
        assert!(mask.contains(ObjectStatusMaskType::DETECTED));
    }

    #[test]
    fn object_status_parse_tokens_additive() {
        let mask = ObjectStatusMaskType::parse_tokens(["+STEALTHED", "+DETECTED"].iter().copied())
            .expect("parse succeeds");
        assert!(mask.contains(ObjectStatusMaskType::STEALTHED));
        assert!(mask.contains(ObjectStatusMaskType::DETECTED));

        let cleared =
            ObjectStatusMaskType::parse_tokens(["+STEALTHED", "-STEALTHED"].iter().copied())
                .expect("parse succeeds");
        assert!(!cleared.contains(ObjectStatusMaskType::STEALTHED));
    }

    #[test]
    fn object_status_parse_tokens_none() {
        let mask =
            ObjectStatusMaskType::parse_tokens(["NONE"].iter().copied()).expect("parse succeeds");
        assert!(mask.is_empty());
    }

    #[test]
    fn test_object_status_mask_creation() {
        let mask = make_object_status_mask(ObjectStatusTypes::Destroyed);
        assert!(mask.contains(ObjectStatusMaskType::DESTROYED));

        let mask2 =
            make_object_status_mask2(ObjectStatusTypes::Destroyed, ObjectStatusTypes::Aflame);
        assert!(mask2.contains(ObjectStatusMaskType::DESTROYED));
        assert!(mask2.contains(ObjectStatusMaskType::AFLAME));
    }

    #[test]
    fn test_object_status_mask_operations() {
        let mut mask = ObjectStatusMaskType::empty();
        assert!(!object_status_mask_any_set(&mask));

        mask |= ObjectStatusMaskType::DESTROYED;
        assert!(object_status_mask_any_set(&mask));
        assert!(test_object_status_mask(&mask, ObjectStatusTypes::Destroyed));

        clear_object_status_mask(&mut mask);
        assert!(!object_status_mask_any_set(&mask));
    }

    #[test]
    fn object_status_parse_tokens_errors() {
        let err = ObjectStatusMaskType::parse_tokens(["UNKNOWN"].iter().copied())
            .expect_err("unknown token");
        assert!(
            err.contains("unknown object status"),
            "unexpected error: {err}"
        );

        let err = ObjectStatusMaskType::parse_tokens(["STEALTHED", "+DETECTED"].iter().copied())
            .expect_err("mixed modes");
        assert!(
            err.contains("mix direct"),
            "unexpected error message: {err}"
        );
    }
}

/// Test set and clear flags
pub fn test_object_status_mask_multi(
    mask: &ObjectStatusMaskType,
    must_be_set: &ObjectStatusMaskType,
    must_be_clear: &ObjectStatusMaskType,
) -> bool {
    mask.test_set_and_clear(*must_be_set, *must_be_clear)
}

/// Check if any status bits are set
pub fn object_status_mask_any_set(mask: &ObjectStatusMaskType) -> bool {
    !mask.is_empty()
}

/// Clear all status bits
pub fn clear_object_status_mask(mask: &mut ObjectStatusMaskType) {
    mask.clear_all();
}

/// Set all status bits
pub fn set_all_object_status_mask_bits(mask: &mut ObjectStatusMaskType) {
    mask.set_all();
}

/// Flip all status bits
pub fn flip_object_status_mask(mask: &mut ObjectStatusMaskType) {
    mask.flip_all();
}
