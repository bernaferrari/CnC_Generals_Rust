// Weapon bonus, weapon-set, upgrade, and player masks
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

bitflags! {
    /// Weapon bonus condition flags (matching C++ WeaponBonusConditionFlags)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct WeaponBonusConditionFlags: u32 {
        const GARRISONED = 1 << 0;
        const HORDE = 1 << 1;
        const CONTINUOUS_FIRE_MEAN = 1 << 2;
        const CONTINUOUS_FIRE_FAST = 1 << 3;
        const NATIONALISM = 1 << 4;
        const PLAYER_UPGRADE = 1 << 5;
        const DRONE_SPOTTING = 1 << 6;
        const DEMORALIZED = 1 << 7;
        const DEMORALIZED_OBSOLETE = 1 << 8;
        const ENTHUSIASTIC = 1 << 9;
        const VETERAN = 1 << 10;
        const ELITE = 1 << 11;
        const HERO = 1 << 12;
        const BATTLEPLAN_BOMBARDMENT = 1 << 13;
        const BATTLEPLAN_HOLDTHELINE = 1 << 14;
        const BATTLEPLAN_SEARCHANDDESTROY = 1 << 15;
        const SUBLIMINAL = 1 << 16;
        const SOLO_HUMAN_EASY = 1 << 17;
        const SOLO_HUMAN_NORMAL = 1 << 18;
        const SOLO_HUMAN_HARD = 1 << 19;
        const SOLO_AI_EASY = 1 << 20;
        const SOLO_AI_NORMAL = 1 << 21;
        const SOLO_AI_HARD = 1 << 22;
        const TARGET_FAERIE_FIRE = 1 << 23;
        const FANATICISM = 1 << 24;
        const FRENZY_ONE = 1 << 25;
        const FRENZY_TWO = 1 << 26;
        const FRENZY_THREE = 1 << 27;
        const DRONE_SPOT_FOR_STRIKE = 1 << 28;
    }
}

impl WeaponBonusConditionFlags {
    pub fn new() -> Self {
        Self::none()
    }

    pub fn none() -> Self {
        Self::from_bits_truncate(0)
    }

    /// Clear specific condition flag(s) from the mask
    pub fn clear(&mut self, condition: WeaponBonusConditionType) {
        // Convert WeaponBonusConditionType to the appropriate flag and remove it
        let flag = match condition {
            WeaponBonusConditionType::Invalid => return,
            WeaponBonusConditionType::Garrisoned => Self::GARRISONED,
            WeaponBonusConditionType::Horde => Self::HORDE,
            WeaponBonusConditionType::ContinuousFireMean => Self::CONTINUOUS_FIRE_MEAN,
            WeaponBonusConditionType::ContinuousFireFast => Self::CONTINUOUS_FIRE_FAST,
            WeaponBonusConditionType::Nationalism => Self::NATIONALISM,
            WeaponBonusConditionType::PlayerUpgrade => Self::PLAYER_UPGRADE,
            WeaponBonusConditionType::DroneSpotting => Self::DRONE_SPOTTING,
            WeaponBonusConditionType::Demoralized => Self::DEMORALIZED,
            WeaponBonusConditionType::Elite => Self::ELITE,
            WeaponBonusConditionType::Veteran => Self::VETERAN,
            WeaponBonusConditionType::DroneSpotForStrike => Self::DRONE_SPOT_FOR_STRIKE,
            WeaponBonusConditionType::DemoralizedObsolete => Self::DEMORALIZED_OBSOLETE,
            WeaponBonusConditionType::Enthusiastic => Self::ENTHUSIASTIC,
            WeaponBonusConditionType::Hero => Self::HERO,
            WeaponBonusConditionType::BattlePlanBombardment => Self::BATTLEPLAN_BOMBARDMENT,
            WeaponBonusConditionType::BattlePlanHoldTheLine => Self::BATTLEPLAN_HOLDTHELINE,
            WeaponBonusConditionType::BattlePlanSearchAndDestroy => {
                Self::BATTLEPLAN_SEARCHANDDESTROY
            }
            WeaponBonusConditionType::Subliminal => Self::SUBLIMINAL,
            WeaponBonusConditionType::SoloHumanEasy => Self::SOLO_HUMAN_EASY,
            WeaponBonusConditionType::SoloHumanNormal => Self::SOLO_HUMAN_NORMAL,
            WeaponBonusConditionType::SoloHumanHard => Self::SOLO_HUMAN_HARD,
            WeaponBonusConditionType::SoloAiEasy => Self::SOLO_AI_EASY,
            WeaponBonusConditionType::SoloAiNormal => Self::SOLO_AI_NORMAL,
            WeaponBonusConditionType::SoloAiHard => Self::SOLO_AI_HARD,
            WeaponBonusConditionType::TargetFaerieFire => Self::TARGET_FAERIE_FIRE,
            WeaponBonusConditionType::Fanaticism => Self::FANATICISM,
            WeaponBonusConditionType::FrenzyOne => Self::FRENZY_ONE,
            WeaponBonusConditionType::FrenzyTwo => Self::FRENZY_TWO,
            WeaponBonusConditionType::FrenzyThree => Self::FRENZY_THREE,
        };
        self.remove(flag);
    }

    /// Set a specific condition flag in the mask
    pub fn set_condition(&mut self, condition: WeaponBonusConditionType) {
        let flag = match condition {
            WeaponBonusConditionType::Invalid => return,
            WeaponBonusConditionType::Garrisoned => Self::GARRISONED,
            WeaponBonusConditionType::Horde => Self::HORDE,
            WeaponBonusConditionType::ContinuousFireMean => Self::CONTINUOUS_FIRE_MEAN,
            WeaponBonusConditionType::ContinuousFireFast => Self::CONTINUOUS_FIRE_FAST,
            WeaponBonusConditionType::Nationalism => Self::NATIONALISM,
            WeaponBonusConditionType::PlayerUpgrade => Self::PLAYER_UPGRADE,
            WeaponBonusConditionType::DroneSpotting => Self::DRONE_SPOTTING,
            WeaponBonusConditionType::Demoralized => Self::DEMORALIZED,
            WeaponBonusConditionType::Elite => Self::ELITE,
            WeaponBonusConditionType::Veteran => Self::VETERAN,
            WeaponBonusConditionType::DroneSpotForStrike => Self::DRONE_SPOT_FOR_STRIKE,
            WeaponBonusConditionType::DemoralizedObsolete => Self::DEMORALIZED_OBSOLETE,
            WeaponBonusConditionType::Enthusiastic => Self::ENTHUSIASTIC,
            WeaponBonusConditionType::Hero => Self::HERO,
            WeaponBonusConditionType::BattlePlanBombardment => Self::BATTLEPLAN_BOMBARDMENT,
            WeaponBonusConditionType::BattlePlanHoldTheLine => Self::BATTLEPLAN_HOLDTHELINE,
            WeaponBonusConditionType::BattlePlanSearchAndDestroy => {
                Self::BATTLEPLAN_SEARCHANDDESTROY
            }
            WeaponBonusConditionType::Subliminal => Self::SUBLIMINAL,
            WeaponBonusConditionType::SoloHumanEasy => Self::SOLO_HUMAN_EASY,
            WeaponBonusConditionType::SoloHumanNormal => Self::SOLO_HUMAN_NORMAL,
            WeaponBonusConditionType::SoloHumanHard => Self::SOLO_HUMAN_HARD,
            WeaponBonusConditionType::SoloAiEasy => Self::SOLO_AI_EASY,
            WeaponBonusConditionType::SoloAiNormal => Self::SOLO_AI_NORMAL,
            WeaponBonusConditionType::SoloAiHard => Self::SOLO_AI_HARD,
            WeaponBonusConditionType::TargetFaerieFire => Self::TARGET_FAERIE_FIRE,
            WeaponBonusConditionType::Fanaticism => Self::FANATICISM,
            WeaponBonusConditionType::FrenzyOne => Self::FRENZY_ONE,
            WeaponBonusConditionType::FrenzyTwo => Self::FRENZY_TWO,
            WeaponBonusConditionType::FrenzyThree => Self::FRENZY_THREE,
        };
        self.insert(flag);
    }
}

bitflags! {
    /// Weapon set flags (matching C++ WeaponSetFlags)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct WeaponSetFlags: u32 {
        const PRIMARY_WEAPON = 1 << 0;
        const SECONDARY_WEAPON = 1 << 1;
        const TERTIARY_WEAPON = 1 << 2;
        const PASSENGER_WEAPON = 1 << 3;
        const PLAYER_UPGRADE = 1 << 4;
        const VETERAN = 1 << 5;
        // Add more as needed
    }
}

impl WeaponSetFlags {
    pub fn new() -> Self {
        Self::empty()
    }

    pub fn test(&self, weapon_set_type: WeaponSetType) -> bool {
        match weapon_set_type {
            WeaponSetType::Primary => self.contains(Self::PRIMARY_WEAPON),
            WeaponSetType::Secondary => self.contains(Self::SECONDARY_WEAPON),
            WeaponSetType::Tertiary => self.contains(Self::TERTIARY_WEAPON),
            WeaponSetType::Passenger => self.contains(Self::PASSENGER_WEAPON),
        }
    }
}

bitflags! {
    /// Upgrade mask (matching C++ UpgradeMaskType)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct UpgradeMaskType: u128 {
        // Define upgrade bits as needed
    }
}

impl UpgradeMaskType {
    pub fn none() -> Self {
        Self::empty()
    }
}

bitflags! {
    /// Player mask (matching C++ PlayerMaskType)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PlayerMaskType: u32 {
        const PLAYER_1 = 1 << 0;
        const PLAYER_2 = 1 << 1;
        const PLAYER_3 = 1 << 2;
        const PLAYER_4 = 1 << 3;
        const PLAYER_5 = 1 << 4;
        const PLAYER_6 = 1 << 5;
        const PLAYER_7 = 1 << 6;
        const PLAYER_8 = 1 << 7;
    }
}

impl PlayerMaskType {
    pub fn none() -> Self {
        Self::empty()
    }
}

/// All players mask (matching C++ PLAYERMASK_ALL = 0xffff)
pub const PLAYERMASK_ALL: PlayerMaskType = PlayerMaskType::all();

