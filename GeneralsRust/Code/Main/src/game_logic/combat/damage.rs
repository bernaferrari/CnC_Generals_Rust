// C++ ownership: Damage.h damage/armor identity and host conversion semantics.

/// C++ `DamageType` (Damage.h:26-70) on the live host fire_at → take_damage path.
/// Legacy host names stay as real variants (match-pattern compatible). Missing
/// C++ types are first-class so `map_store_damage_type` no longer collapses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DamageType {
    #[default]
    Bullet,
    Explosive,
    Fire,
    Laser,
    Toxin,
    Radiation,
    EMP,
    /// C++ DAMAGE_MICROWAVE (Damage.h:63) — ordinary HP through armor.
    /// Not IsSubdualDamage. Host EMP remains a leftover alias of this type.
    Microwave,
    Flame,
    Anthrax,
    Unresistable,
    /// C++ DAMAGE_FALLING residual (PhysicsBehavior landing splat).
    Falling,
    /// C++ DAMAGE_STATUS residual (doStatusDamage; amount = duration msec).
    Status,
    /// C++ DAMAGE_KILL_PILOT residual (vehicle unmanned; no HP).
    KillPilot,
    /// C++ DAMAGE_DISARM residual (safe mine clear without detonation).
    Disarm,
    /// C++ DAMAGE_DEPLOY residual (AssaultTransport beginAssault; no HP).
    Deploy,
    /// C++ DAMAGE_HACK residual (timer-based hack; no HP on fire).
    Hack,
    /// C++ DAMAGE_SURRENDER (Damage.h:42). Retail ALLOW_SURRENDER is off;
    /// lethal hits deal normal HP (ActiveBody.cpp:517-537 compiled out).
    Surrender,
    /// C++ DAMAGE_PENALTY residual (game-rule HP damage; no radar event).
    Penalty,
    /// C++ DAMAGE_KILL_GARRISONED residual (kill floor(amount) occupants; no structure HP).
    KillGarrisoned,
    /// C++ DAMAGE_HEALING residual (attemptHealing; amount restores HP, never destroys).
    Healing,
    /// C++ DAMAGE_WATER residual (underwater / waveguide HP damage; no dusty FX).
    Water,
    /// C++ DAMAGE_CRUSH residual (SquishCollide / PhysicsUpdate crush).
    Crush,
    ArmorPiercing,
    Gattling,
    Sniper,
    Melee,
    HazardCleanup,
    ParticleBeam,
    Toppling,
    InfantryMissile,
    AuroraBomb,
    LandMine,
    JetMissiles,
    StealthJetMissiles,
    MolotovCocktail,
    ComancheVulcan,
    SubdualMissile,
    SubdualVehicle,
    SubdualBuilding,
    SubdualUnresistable,
}

impl DamageType {
    /// Crate / C++ ordinal identity. `DamageNumTypes` is the count, not a payload.
    #[inline]
    pub fn from_store(dt: gamelogic::damage::DamageType) -> Self {
        use gamelogic::damage::DamageType as G;
        match dt {
            G::Explosion => Self::Explosive,
            G::Crush => Self::Crush,
            G::ArmorPiercing => Self::ArmorPiercing,
            G::SmallArms => Self::Bullet,
            G::Gattling => Self::Gattling,
            G::Radiation => Self::Radiation,
            G::Flame => Self::Flame,
            G::Laser => Self::Laser,
            G::Sniper => Self::Sniper,
            G::Poison => Self::Toxin,
            G::Healing => Self::Healing,
            G::Unresistable => Self::Unresistable,
            G::Water => Self::Water,
            G::Deploy => Self::Deploy,
            G::Surrender => Self::Surrender,
            G::Hack => Self::Hack,
            G::KillPilot => Self::KillPilot,
            G::Penalty => Self::Penalty,
            G::Falling => Self::Falling,
            G::Melee => Self::Melee,
            G::Disarm => Self::Disarm,
            G::HazardCleanup => Self::HazardCleanup,
            G::ParticleBeam => Self::ParticleBeam,
            G::Toppling => Self::Toppling,
            G::InfantryMissile => Self::InfantryMissile,
            G::AuroraBomb => Self::AuroraBomb,
            G::LandMine => Self::LandMine,
            G::JetMissiles => Self::JetMissiles,
            G::StealthJetMissiles => Self::StealthJetMissiles,
            G::MolotovCocktail => Self::MolotovCocktail,
            G::ComancheVulcan => Self::ComancheVulcan,
            G::SubdualMissile => Self::SubdualMissile,
            G::SubdualVehicle => Self::SubdualVehicle,
            G::SubdualBuilding => Self::SubdualBuilding,
            G::SubdualUnresistable => Self::SubdualUnresistable,
            G::Microwave => Self::Microwave,
            G::KillGarrisoned => Self::KillGarrisoned,
            G::Status => Self::Status,
            G::DamageNumTypes => Self::Unresistable,
        }
    }

    #[inline]
    pub fn to_store(self) -> gamelogic::damage::DamageType {
        use gamelogic::damage::DamageType as G;
        match self {
            Self::Bullet => G::SmallArms,
            Self::Explosive => G::Explosion,
            Self::Fire | Self::Flame => G::Flame,
            Self::Laser => G::Laser,
            Self::Toxin | Self::Anthrax => G::Poison,
            Self::Radiation => G::Radiation,
            Self::EMP | Self::Microwave => G::Microwave,
            Self::Unresistable => G::Unresistable,
            Self::Falling => G::Falling,
            Self::Status => G::Status,
            Self::KillPilot => G::KillPilot,
            Self::Disarm => G::Disarm,
            Self::Deploy => G::Deploy,
            Self::Hack => G::Hack,
            Self::Surrender => G::Surrender,
            Self::Penalty => G::Penalty,
            Self::KillGarrisoned => G::KillGarrisoned,
            Self::Healing => G::Healing,
            Self::Water => G::Water,
            Self::Crush => G::Crush,
            Self::ArmorPiercing => G::ArmorPiercing,
            Self::Gattling => G::Gattling,
            Self::Sniper => G::Sniper,
            Self::Melee => G::Melee,
            Self::HazardCleanup => G::HazardCleanup,
            Self::ParticleBeam => G::ParticleBeam,
            Self::Toppling => G::Toppling,
            Self::InfantryMissile => G::InfantryMissile,
            Self::AuroraBomb => G::AuroraBomb,
            Self::LandMine => G::LandMine,
            Self::JetMissiles => G::JetMissiles,
            Self::StealthJetMissiles => G::StealthJetMissiles,
            Self::MolotovCocktail => G::MolotovCocktail,
            Self::ComancheVulcan => G::ComancheVulcan,
            Self::SubdualMissile => G::SubdualMissile,
            Self::SubdualVehicle => G::SubdualVehicle,
            Self::SubdualBuilding => G::SubdualBuilding,
            Self::SubdualUnresistable => G::SubdualUnresistable,
        }
    }

    /// C++ `IsSubdualDamage` (Damage.h:95-107).
    #[inline]
    pub fn is_subdual(self) -> bool {
        matches!(
            self,
            Self::SubdualMissile
                | Self::SubdualVehicle
                | Self::SubdualBuilding
                | Self::SubdualUnresistable
        )
    }

    /// C++ `IsHealthDamagingDamage` (Damage.h:110-127).
    #[inline]
    pub fn is_health_damaging(self) -> bool {
        !matches!(
            self,
            Self::Status
                | Self::SubdualMissile
                | Self::SubdualVehicle
                | Self::SubdualBuilding
                | Self::SubdualUnresistable
                | Self::KillPilot
                | Self::KillGarrisoned
        )
    }
}

/// Armor types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArmorType {
    None,
    Infantry,
    Vehicle,
    Aircraft,
    Structure,
    Flame,
}

/// Damage calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DamageResult {
    pub final_damage: f32,
    pub damage_type: DamageType,
    pub was_critical: bool,
    pub armor_reduction: f32,
}
