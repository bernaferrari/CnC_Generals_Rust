// Object status, special-power, and disabled masks
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

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

        let legacy_mask =
            legacy_object_status::ObjectStatusMaskType::parse_tokens(tokens.iter().copied())?;
        Ok(Self::from_bits_retain(legacy_mask.bits()))
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

bitflags! {
    /// Special power mask (matching C++ SpecialPowerMaskType)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SpecialPowerMaskType: u32 {
        const SUPERWEAPON_A = 1 << 0;
        const SUPERWEAPON_B = 1 << 1;
        const SUPERWEAPON_C = 1 << 2;
        const CASH_HACK = 1 << 3;
        const RADAR_VAN_SCAN = 1 << 4;
        const SPY_SATELLITE = 1 << 5;
        const DISGUISE = 1 << 6;
        const RADAR_JAMMER = 1 << 7;
        // Add more as needed
    }
}

#[cfg(test)]
mod tests {
    use super::{
        engine_geometry_to_logic, geometry_type_from_u32, geometry_type_to_u32, EngineGeometryInfo,
        EngineGeometryType, GeometryExtentModType, GeometryInfo, ObjectStatusMaskType,
    };

    #[test]
    fn object_status_parse_tokens_matches_legacy_helper() {
        let mask = ObjectStatusMaskType::parse_tokens(["STEALTHED", "DETECTED"].iter().copied())
            .expect("parse succeeds");
        assert!(mask.contains(ObjectStatusMaskType::STEALTHED));
        assert!(mask.contains(ObjectStatusMaskType::DETECTED));
        assert!(!mask.contains(ObjectStatusMaskType::AFLAME));
    }

    #[test]
    fn object_status_parse_tokens_accepts_additive_modifiers() {
        let mask = ObjectStatusMaskType::parse_tokens(
            ["+STEALTHED", "+DETECTED", "-STEALTHED"].iter().copied(),
        )
        .expect("parse succeeds");
        assert!(mask.contains(ObjectStatusMaskType::DETECTED));
        assert!(!mask.contains(ObjectStatusMaskType::STEALTHED));
    }

    #[test]
    fn object_status_parse_tokens_errors_on_mixed_none() {
        let err = ObjectStatusMaskType::parse_tokens(["NONE", "STEALTHED"].iter().copied())
            .expect_err("mixing NONE with other tokens is invalid");
        assert!(
            err.contains("NONE"),
            "error message should reference NONE token"
        );
    }

    #[test]
    fn geometry_info_tweak_extents_cycles_type_and_clears_small_flag() {
        let mut geometry = GeometryInfo::default();
        geometry.geometry_type = EngineGeometryType::Box;
        geometry.is_small = true;
        geometry.tweak_extents(GeometryExtentModType::Type, 1.0);

        assert_eq!(geometry.geometry_type, EngineGeometryType::Sphere);
        assert!(!geometry.is_small);
    }

    #[test]
    fn geometry_type_round_trip_helpers_match_cpp_order() {
        assert_eq!(geometry_type_to_u32(EngineGeometryType::Sphere), 0);
        assert_eq!(geometry_type_to_u32(EngineGeometryType::Cylinder), 1);
        assert_eq!(geometry_type_to_u32(EngineGeometryType::Box), 2);

        assert_eq!(geometry_type_from_u32(0), EngineGeometryType::Sphere);
        assert_eq!(geometry_type_from_u32(1), EngineGeometryType::Cylinder);
        assert_eq!(geometry_type_from_u32(2), EngineGeometryType::Box);
    }

    #[test]
    fn engine_geometry_to_logic_preserves_type_and_small_flag() {
        let engine_geometry =
            EngineGeometryInfo::new(EngineGeometryType::Cylinder, true, 12.0, 8.0, 4.0);

        let logic_geometry = engine_geometry_to_logic(&engine_geometry);

        assert_eq!(logic_geometry.geometry_type, EngineGeometryType::Cylinder);
        assert!(logic_geometry.is_small);
        assert_eq!(logic_geometry.get_major_radius(), 6.0);
        assert_eq!(logic_geometry.get_minor_radius(), 2.0);
        assert_eq!(logic_geometry.get_max_height_above_position(), 8.0);
    }
}

impl SpecialPowerMaskType {
    pub fn none() -> Self {
        Self::empty()
    }
}

bitflags! {
    /// Disabled mask (matching C++ DisabledMaskType)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisabledMaskType: u32 {
        const DISABLED_DEFAULT = 1 << 0;
        const DISABLED_HACKED = 1 << 1;
        const DISABLED_EMP = 1 << 2;
        const HELD = 1 << 3;
        const PARALYZED = 1 << 4;
        const DISABLED_UNMANNED = 1 << 5;
        const DISABLED_UNDERPOWERED = 1 << 6;
        const DISABLED_FREEFALL = 1 << 7;
        const DISABLED_AWESTRUCK = 1 << 8;
        const DISABLED_BRAINWASHED = 1 << 9;
        const DISABLED_SUBDUED = 1 << 10;
        const DISABLED_SCRIPT_DISABLED = 1 << 11;
        const DISABLED_SCRIPT_UNDERPOWERED = 1 << 12;
    }
}

impl DisabledMaskType {
    pub fn none() -> Self {
        Self::empty()
    }

    pub fn any(&self) -> bool {
        !self.is_empty()
    }

    pub fn test(&self, disabled_type: DisabledType) -> bool {
        match disabled_type {
            DisabledType::DisabledDefault => self.contains(Self::DISABLED_DEFAULT),
            DisabledType::DisabledHacked => self.contains(Self::DISABLED_HACKED),
            DisabledType::DisabledEmp => self.contains(Self::DISABLED_EMP),
            DisabledType::Held => self.contains(Self::HELD),
            DisabledType::Paralyzed => self.contains(Self::PARALYZED),
            DisabledType::DisabledSubdued => self.contains(Self::DISABLED_SUBDUED),
            DisabledType::DisabledUnmanned | DisabledType::Unmanned => {
                self.contains(Self::DISABLED_UNMANNED)
            }
            DisabledType::DisabledUnderpowered => self.contains(Self::DISABLED_UNDERPOWERED),
            DisabledType::DisabledFreefall => self.contains(Self::DISABLED_FREEFALL),
            DisabledType::DisabledAwestruck => self.contains(Self::DISABLED_AWESTRUCK),
            DisabledType::DisabledBrainwashed => self.contains(Self::DISABLED_BRAINWASHED),
            DisabledType::DisabledScriptDisabled => self.contains(Self::DISABLED_SCRIPT_DISABLED),
            DisabledType::DisabledScriptUnderpowered => {
                self.contains(Self::DISABLED_SCRIPT_UNDERPOWERED)
            }
            DisabledType::DisabledAny => self.any(),
        }
    }

    pub fn set_disabled(&mut self, disabled_type: DisabledType) {
        match disabled_type {
            DisabledType::DisabledDefault => *self |= Self::DISABLED_DEFAULT,
            DisabledType::DisabledHacked => *self |= Self::DISABLED_HACKED,
            DisabledType::DisabledEmp => *self |= Self::DISABLED_EMP,
            DisabledType::Held => *self |= Self::HELD,
            DisabledType::Paralyzed => *self |= Self::PARALYZED,
            DisabledType::DisabledSubdued => *self |= Self::DISABLED_SUBDUED,
            DisabledType::DisabledUnmanned | DisabledType::Unmanned => {
                *self |= Self::DISABLED_UNMANNED
            }
            DisabledType::DisabledUnderpowered => *self |= Self::DISABLED_UNDERPOWERED,
            DisabledType::DisabledFreefall => *self |= Self::DISABLED_FREEFALL,
            DisabledType::DisabledAwestruck => *self |= Self::DISABLED_AWESTRUCK,
            DisabledType::DisabledBrainwashed => *self |= Self::DISABLED_BRAINWASHED,
            DisabledType::DisabledScriptDisabled => *self |= Self::DISABLED_SCRIPT_DISABLED,
            DisabledType::DisabledScriptUnderpowered => *self |= Self::DISABLED_SCRIPT_UNDERPOWERED,
            DisabledType::DisabledAny => {} // No-op for aggregated state
        }
    }

    pub fn clear(&mut self, disabled_type: DisabledType) {
        match disabled_type {
            DisabledType::DisabledDefault => *self &= !Self::DISABLED_DEFAULT,
            DisabledType::DisabledHacked => *self &= !Self::DISABLED_HACKED,
            DisabledType::DisabledEmp => *self &= !Self::DISABLED_EMP,
            DisabledType::Held => *self &= !Self::HELD,
            DisabledType::Paralyzed => *self &= !Self::PARALYZED,
            DisabledType::DisabledSubdued => *self &= !Self::DISABLED_SUBDUED,
            DisabledType::DisabledUnmanned | DisabledType::Unmanned => {
                *self &= !Self::DISABLED_UNMANNED
            }
            DisabledType::DisabledUnderpowered => *self &= !Self::DISABLED_UNDERPOWERED,
            DisabledType::DisabledFreefall => *self &= !Self::DISABLED_FREEFALL,
            DisabledType::DisabledAwestruck => *self &= !Self::DISABLED_AWESTRUCK,
            DisabledType::DisabledBrainwashed => *self &= !Self::DISABLED_BRAINWASHED,
            DisabledType::DisabledScriptDisabled => *self &= !Self::DISABLED_SCRIPT_DISABLED,
            DisabledType::DisabledScriptUnderpowered => {
                *self &= !Self::DISABLED_SCRIPT_UNDERPOWERED
            }
            DisabledType::DisabledAny => *self = Self::empty(),
        }
    }
}

/// Type alias for backward compatibility with C++ naming
pub type DisabledMask = DisabledMaskType;

/// ID type for ThingTemplates
pub type ThingTemplateId = u32;

/// ID type for UpgradeTemplates
pub type UpgradeTemplateId = u32;

/// Production ID for tracking unit construction
pub type ProductionID = u32;

/// Invalid production ID constant
pub const PRODUCTIONID_INVALID: ProductionID = 0;

