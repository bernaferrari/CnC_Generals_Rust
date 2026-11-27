use serde::{Deserialize, Serialize};

/// Death outcome carried by live objects and native saves.
/// Its variant order preserves the native save format. `ordinal` supplies the
/// C++ DeathType identity used by authored filters.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename = "HostDeathType")]
pub enum DeathOutcome {
    /// Default combat / generic residual death (passes ALL -CRUSHED -SPLATTED).
    #[default]
    Normal,
    /// DEATH_NONE residual.
    None,
    /// DEATH_CRUSHED residual — blocked by EjectPilotDie DeathTypes filter.
    Crushed,
    /// DEATH_BURNED residual (flame weapons).
    Burned,
    /// DEATH_EXPLODED residual (missiles / bombs).
    Exploded,
    /// DEATH_POISONED residual.
    Poisoned,
    /// DEATH_TOPPLED residual.
    Toppled,
    /// DEATH_FLOODED residual.
    Flooded,
    /// DEATH_SUICIDED residual (terrorist / demo).
    Suicided,
    /// DEATH_LASERED residual.
    Lasered,
    /// DEATH_DETONATED residual.
    Detonated,
    /// DEATH_SPLATTED residual — blocked by EjectPilotDie DeathTypes filter.
    Splatted,
    /// DEATH_POISONED_BETA residual.
    PoisonedBeta,
    /// DEATH_POISONED_GAMMA residual.
    PoisonedGamma,
    /// CPP DEATH_EXTRA_4: Battle Bus empty-hulk penalty selects another die module.
    Extra4,
}

impl DeathOutcome {
    /// Map gamelogic / Weapon.ini DeathType residual into host DieMux death class.
    pub fn from_store(dt: super::DeathType) -> Self {
        use super::DeathType as G;
        match dt {
            G::Normal => Self::Normal,
            G::None => Self::None,
            G::Crushed => Self::Crushed,
            G::Burned => Self::Burned,
            G::Exploded => Self::Exploded,
            G::Poisoned => Self::Poisoned,
            G::Toppled => Self::Toppled,
            G::Flooded => Self::Flooded,
            G::Suicided => Self::Suicided,
            G::Lasered => Self::Lasered,
            G::Detonated => Self::Detonated,
            G::Splatted => Self::Splatted,
            G::PoisonedBeta => Self::PoisonedBeta,
            G::PoisonedGamma => Self::PoisonedGamma,
            G::Extra4 => Self::Extra4,
            _ => Self::Normal,
        }
    }

    /// Coarse DamageType → DeathType residual when weapon DeathType is unset/Normal.
    pub fn from_host_damage_type(dt: super::CombatDamage) -> Self {
        use super::CombatDamage as H;
        match dt {
            H::Flame | H::Fire | H::MolotovCocktail => Self::Burned,
            H::Laser | H::ParticleBeam => Self::Lasered,
            H::Toxin | H::Anthrax => Self::Poisoned,
            H::Explosive | H::AuroraBomb | H::LandMine => Self::Exploded,
            H::Radiation => Self::Detonated,
            H::Falling => Self::Splatted,
            H::Crush => Self::Crushed,
            H::Toppling => Self::Toppled,
            _ => Self::Normal,
        }
    }

    pub fn as_name(self) -> &'static str {
        super::DEATH_TYPE_NAMES[self.ordinal() as usize]
    }

    #[inline]
    pub fn ordinal(self) -> u8 {
        match self {
            DeathOutcome::Normal => 0,
            DeathOutcome::None => 1,
            DeathOutcome::Crushed => 2,
            DeathOutcome::Burned => 3,
            DeathOutcome::Exploded => 4,
            DeathOutcome::Poisoned => 5,
            DeathOutcome::Toppled => 6,
            DeathOutcome::Flooded => 7,
            DeathOutcome::Suicided => 8,
            DeathOutcome::Lasered => 9,
            DeathOutcome::Detonated => 10,
            DeathOutcome::Splatted => 11,
            DeathOutcome::PoisonedBeta => 12,
            // C++ Damage.h:165-172 EXTRA_2..EXTRA_8 = 13..19, POISONED_GAMMA = 20.
            DeathOutcome::PoisonedGamma => 20,
            DeathOutcome::Extra4 => 15,
        }
    }

    #[inline]
    pub fn from_ordinal(value: u8) -> Self {
        Self::from_store(super::DeathType::from_u32(u32::from(value)))
    }
}
