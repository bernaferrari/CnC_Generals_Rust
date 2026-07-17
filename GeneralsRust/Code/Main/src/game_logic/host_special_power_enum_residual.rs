//! Wave 80: SpecialPower enum residual discriminants.
//!
//! Freezes C++ `SpecialPowerType.h` / `SpecialPowerMaskType::s_bitNameList`
//! residual for host superweapon kinds + full ordered name table length.
//!
//! - `SPECIALPOWER_COUNT` residual **67** (Invalid..Battleship + Count).
//! - Host superweapon → retail `SPECIAL_*` / faction variant names.
//! - Host `command_system::SpecialPowerType` → C++ enum name residual bridge.
//!
//! Fail-closed:
//! - Not full SpecialPowerStore INI load / save ordinal rebind matrix
//! - Not full SpecialPowerMask bit ops across all 67 discriminants in host
//! - Shell `playable_claim` stays false; network deferred

use crate::command_system::SpecialPowerType as HostCommandSpecialPowerType;
use crate::game_logic::special_power_strikes::HostSuperweaponKind;

/// C++ `SPECIALPOWER_COUNT` residual (SpecialPowerType.h).
pub const SPECIALPOWER_COUNT: u32 = 67;

/// Ordered C++ `s_bitNameList` residual (SpecialPower.cpp), excluding trailing NULL.
/// Length **67** names (indices 0..66); Count sentinel is not a bit name.
pub const SPECIAL_POWER_BIT_NAME_LIST: &[&str] = &[
    "SPECIAL_INVALID",
    // Superweapons
    "SPECIAL_DAISY_CUTTER",
    "SPECIAL_PARADROP_AMERICA",
    "SPECIAL_CARPET_BOMB",
    "SPECIAL_CLUSTER_MINES",
    "SPECIAL_EMP_PULSE",
    "SPECIAL_NAPALM_STRIKE",
    "SPECIAL_CASH_HACK",
    "SPECIAL_NEUTRON_MISSILE",
    "SPECIAL_SPY_SATELLITE",
    "SPECIAL_DEFECTOR",
    "SPECIAL_TERROR_CELL",
    "SPECIAL_AMBUSH",
    "SPECIAL_BLACK_MARKET_NUKE",
    "SPECIAL_ANTHRAX_BOMB",
    "SPECIAL_SCUD_STORM",
    "SPECIAL_DEMORALIZE_OBSOLETE", // retail ZH without ALLOW_DEMORALIZE
    "SPECIAL_CRATE_DROP",
    "SPECIAL_A10_THUNDERBOLT_STRIKE",
    "SPECIAL_DETONATE_DIRTY_NUKE",
    "SPECIAL_ARTILLERY_BARRAGE",
    // Special abilities
    "SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES",
    "SPECIAL_REMOTE_CHARGES",
    "SPECIAL_TIMED_CHARGES",
    "SPECIAL_HELIX_NAPALM_BOMB",
    "SPECIAL_HACKER_DISABLE_BUILDING",
    "SPECIAL_TANKHUNTER_TNT_ATTACK",
    "SPECIAL_BLACKLOTUS_CAPTURE_BUILDING",
    "SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK",
    "SPECIAL_BLACKLOTUS_STEAL_CASH_HACK",
    "SPECIAL_INFANTRY_CAPTURE_BUILDING",
    "SPECIAL_RADAR_VAN_SCAN",
    "SPECIAL_SPY_DRONE",
    "SPECIAL_DISGUISE_AS_VEHICLE",
    "SPECIAL_BOOBY_TRAP",
    "SPECIAL_REPAIR_VEHICLES",
    "SPECIAL_PARTICLE_UPLINK_CANNON",
    "SPECIAL_CASH_BOUNTY",
    "SPECIAL_CHANGE_BATTLE_PLANS",
    "SPECIAL_CIA_INTELLIGENCE",
    "SPECIAL_CLEANUP_AREA",
    "SPECIAL_LAUNCH_BAIKONUR_ROCKET",
    "SPECIAL_SPECTRE_GUNSHIP",
    "SPECIAL_GPS_SCRAMBLER",
    "SPECIAL_FRENZY",
    "SPECIAL_SNEAK_ATTACK",
    // Faction / shortcut variants
    "SPECIAL_CHINA_CARPET_BOMB",
    "EARLY_SPECIAL_CHINA_CARPET_BOMB",
    "SPECIAL_LEAFLET_DROP",
    "EARLY_SPECIAL_LEAFLET_DROP",
    "EARLY_SPECIAL_FRENZY",
    "SPECIAL_COMMUNICATIONS_DOWNLOAD",
    "EARLY_SPECIAL_REPAIR_VEHICLES",
    "SPECIAL_TANK_PARADROP",
    "SUPW_SPECIAL_PARTICLE_UPLINK_CANNON",
    "AIRF_SPECIAL_DAISY_CUTTER",
    "NUKE_SPECIAL_CLUSTER_MINES",
    "NUKE_SPECIAL_NEUTRON_MISSILE",
    "AIRF_SPECIAL_A10_THUNDERBOLT_STRIKE",
    "AIRF_SPECIAL_SPECTRE_GUNSHIP",
    "INFA_SPECIAL_PARADROP_AMERICA",
    "SLTH_SPECIAL_GPS_SCRAMBLER",
    "AIRF_SPECIAL_CARPET_BOMB",
    "SUPR_SPECIAL_CRUISE_MISSILE",
    "LAZR_SPECIAL_PARTICLE_UPLINK_CANNON",
    "SUPW_SPECIAL_NEUTRON_MISSILE",
    "SPECIAL_BATTLESHIP_BOMBARDMENT",
];

impl HostSuperweaponKind {
    /// Baseline C++ SpecialPowerType enum residual name for this host kind.
    pub fn cpp_special_power_enum_name(self) -> &'static str {
        match self {
            HostSuperweaponKind::DaisyCutter => "SPECIAL_DAISY_CUTTER",
            HostSuperweaponKind::A10Strike => "SPECIAL_A10_THUNDERBOLT_STRIKE",
            HostSuperweaponKind::ScudStorm => "SPECIAL_SCUD_STORM",
            HostSuperweaponKind::ParticleCannon => "SPECIAL_PARTICLE_UPLINK_CANNON",
            HostSuperweaponKind::NuclearMissile => "SPECIAL_NEUTRON_MISSILE",
            HostSuperweaponKind::AnthraxBomb => "SPECIAL_ANTHRAX_BOMB",
            HostSuperweaponKind::SpectreGunship => "SPECIAL_SPECTRE_GUNSHIP",
            HostSuperweaponKind::CarpetBomb => "SPECIAL_CARPET_BOMB",
            HostSuperweaponKind::ArtilleryBarrage => "SPECIAL_ARTILLERY_BARRAGE",
            HostSuperweaponKind::CruiseMissile => "SUPR_SPECIAL_CRUISE_MISSILE",
        }
    }

    /// C++ discriminant ordinal residual (matches SpecialPowerType.h / s_bitNameList index).
    pub fn cpp_special_power_ordinal(self) -> u32 {
        match self {
            HostSuperweaponKind::DaisyCutter => 1,
            HostSuperweaponKind::CarpetBomb => 3,
            HostSuperweaponKind::NuclearMissile => 8,
            HostSuperweaponKind::AnthraxBomb => 14,
            HostSuperweaponKind::ScudStorm => 15,
            HostSuperweaponKind::A10Strike => 18,
            HostSuperweaponKind::ArtilleryBarrage => 20,
            HostSuperweaponKind::ParticleCannon => 36,
            HostSuperweaponKind::SpectreGunship => 42,
            HostSuperweaponKind::CruiseMissile => 63,
        }
    }
}

/// Map host command-system power type to C++ SPECIAL_* residual name when known.
pub fn host_command_power_cpp_enum_name(
    power: &HostCommandSpecialPowerType,
) -> Option<&'static str> {
    match power {
        HostCommandSpecialPowerType::DaisyCutter | HostCommandSpecialPowerType::FuelAirBomb => {
            Some("SPECIAL_DAISY_CUTTER")
        }
        HostCommandSpecialPowerType::Airstrike => Some("SPECIAL_A10_THUNDERBOLT_STRIKE"),
        HostCommandSpecialPowerType::Artillery => Some("SPECIAL_ARTILLERY_BARRAGE"),
        HostCommandSpecialPowerType::CarpetBomb => Some("SPECIAL_CARPET_BOMB"),
        HostCommandSpecialPowerType::EarlyChinaCarpetBomb => {
            Some("EARLY_SPECIAL_CHINA_CARPET_BOMB")
        }
        HostCommandSpecialPowerType::AirForceCarpetBomb => Some("AIRF_SPECIAL_CARPET_BOMB"),
        HostCommandSpecialPowerType::ClusterMines => Some("SPECIAL_CLUSTER_MINES"),
        HostCommandSpecialPowerType::CrateDrop => Some("SPECIAL_CRATE_DROP"),
        HostCommandSpecialPowerType::EmergencyRepair => Some("SPECIAL_REPAIR_VEHICLES"),
        HostCommandSpecialPowerType::EarlyEmergencyRepair => Some("EARLY_SPECIAL_REPAIR_VEHICLES"),
        HostCommandSpecialPowerType::NapalmStrike => Some("SPECIAL_NAPALM_STRIKE"),
        HostCommandSpecialPowerType::NuclearMissile => Some("SPECIAL_NEUTRON_MISSILE"),
        HostCommandSpecialPowerType::BlackMarketNuke => Some("SPECIAL_BLACK_MARKET_NUKE"),
        HostCommandSpecialPowerType::DetonateDirtyNuke => Some("SPECIAL_DETONATE_DIRTY_NUKE"),
        HostCommandSpecialPowerType::Paradrop => Some("SPECIAL_PARADROP_AMERICA"),
        HostCommandSpecialPowerType::InfantryParadrop => Some("INFA_SPECIAL_PARADROP_AMERICA"),
        HostCommandSpecialPowerType::TankParadrop => Some("SPECIAL_TANK_PARADROP"),
        HostCommandSpecialPowerType::Ambush => Some("SPECIAL_AMBUSH"),
        HostCommandSpecialPowerType::TerrorCell => Some("SPECIAL_TERROR_CELL"),
        HostCommandSpecialPowerType::ParticleCannon => Some("SPECIAL_PARTICLE_UPLINK_CANNON"),
        HostCommandSpecialPowerType::RadarScan => Some("SPECIAL_RADAR_VAN_SCAN"),
        HostCommandSpecialPowerType::ScudStorm => Some("SPECIAL_SCUD_STORM"),
        HostCommandSpecialPowerType::SpySatellite => Some("SPECIAL_SPY_SATELLITE"),
        HostCommandSpecialPowerType::CiaIntelligence => Some("SPECIAL_CIA_INTELLIGENCE"),
        HostCommandSpecialPowerType::CommunicationsDownload => {
            Some("SPECIAL_COMMUNICATIONS_DOWNLOAD")
        }
        HostCommandSpecialPowerType::SpyDrone => Some("SPECIAL_SPY_DRONE"),
        HostCommandSpecialPowerType::AnthraxBomb => Some("SPECIAL_ANTHRAX_BOMB"),
        HostCommandSpecialPowerType::SpectreGunship => Some("SPECIAL_SPECTRE_GUNSHIP"),
        HostCommandSpecialPowerType::EmpPulse => Some("SPECIAL_EMP_PULSE"),
        HostCommandSpecialPowerType::Frenzy => Some("SPECIAL_FRENZY"),
        HostCommandSpecialPowerType::EarlyFrenzy => Some("EARLY_SPECIAL_FRENZY"),
        HostCommandSpecialPowerType::BattlePlanBombardment
        | HostCommandSpecialPowerType::BattlePlanHoldTheLine
        | HostCommandSpecialPowerType::BattlePlanSearchAndDestroy => {
            Some("SPECIAL_CHANGE_BATTLE_PLANS")
        }
        HostCommandSpecialPowerType::GpsScrambler => Some("SPECIAL_GPS_SCRAMBLER"),
        HostCommandSpecialPowerType::LeafletDrop => Some("SPECIAL_LEAFLET_DROP"),
        HostCommandSpecialPowerType::EarlyLeafletDrop => Some("EARLY_SPECIAL_LEAFLET_DROP"),
        HostCommandSpecialPowerType::SneakAttack => Some("SPECIAL_SNEAK_ATTACK"),
        HostCommandSpecialPowerType::CruiseMissile => Some("SUPR_SPECIAL_CRUISE_MISSILE"),
        HostCommandSpecialPowerType::CleanupArea => Some("SPECIAL_CLEANUP_AREA"),
        HostCommandSpecialPowerType::HelixNapalmBomb => Some("SPECIAL_HELIX_NAPALM_BOMB"),
        // Retail Nuke_SpecialAbilityHelixNukeBomb shares SPECIAL_HELIX_NAPALM_BOMB enum.
        HostCommandSpecialPowerType::HelixNukeBomb => Some("SPECIAL_HELIX_NAPALM_BOMB"),
        HostCommandSpecialPowerType::CashHack => Some("SPECIAL_CASH_HACK"),
        HostCommandSpecialPowerType::AirForceDaisyCutter => Some("AIRF_SPECIAL_DAISY_CUTTER"),
        HostCommandSpecialPowerType::AirForceAirstrike => {
            Some("AIRF_SPECIAL_A10_THUNDERBOLT_STRIKE")
        }
        HostCommandSpecialPowerType::AirForceSpectreGunship => Some("AIRF_SPECIAL_SPECTRE_GUNSHIP"),
        HostCommandSpecialPowerType::SuperweaponParticleCannon => {
            Some("SUPW_SPECIAL_PARTICLE_UPLINK_CANNON")
        }
        HostCommandSpecialPowerType::NukeNeutronMissile => Some("NUKE_SPECIAL_NEUTRON_MISSILE"),
        HostCommandSpecialPowerType::SuperweaponNeutronMissile => {
            Some("SUPW_SPECIAL_NEUTRON_MISSILE")
        }
        HostCommandSpecialPowerType::NukeChinaCarpetBomb => Some("EARLY_SPECIAL_CHINA_CARPET_BOMB"),
        HostCommandSpecialPowerType::StealthGpsScrambler => Some("SPECIAL_GPS_SCRAMBLER"),
        HostCommandSpecialPowerType::BaikonurRocket => Some("SPECIAL_LAUNCH_BAIKONUR_ROCKET"),
        HostCommandSpecialPowerType::NukeDrop => Some("NUKE_SPECIAL_CLUSTER_MINES"),
        HostCommandSpecialPowerType::BattleshipBombardment => {
            Some("SPECIAL_BATTLESHIP_BOMBARDMENT")
        }
        HostCommandSpecialPowerType::LaserCannon => Some("LAZR_SPECIAL_PARTICLE_UPLINK_CANNON"),
        HostCommandSpecialPowerType::MissileDefenderLaserGuided => {
            Some("SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES")
        }
        HostCommandSpecialPowerType::TankHunterTnt => Some("SPECIAL_TANKHUNTER_TNT_ATTACK"),
        HostCommandSpecialPowerType::LaserGuidedHowitzer => {
            Some("SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES")
        }
        HostCommandSpecialPowerType::DemoRebelTimedCharges
        | HostCommandSpecialPowerType::DemoKellTimedCharges
        | HostCommandSpecialPowerType::DemoKellStickyCharges
        | HostCommandSpecialPowerType::BattleBusDemoTrapRollout
        | HostCommandSpecialPowerType::BurtonTimedCharges => Some("SPECIAL_TIMED_CHARGES"),
        HostCommandSpecialPowerType::DemoKellRemoteCharges
        | HostCommandSpecialPowerType::BurtonRemoteCharges => Some("SPECIAL_REMOTE_CHARGES"),
        HostCommandSpecialPowerType::HackerDisableBuilding
        | HostCommandSpecialPowerType::MicrowaveDisableBuilding => {
            Some("SPECIAL_HACKER_DISABLE_BUILDING")
        }
        HostCommandSpecialPowerType::BlackLotusDisableVehicle => {
            Some("SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK")
        }
        HostCommandSpecialPowerType::BlackLotusStealCash => {
            Some("SPECIAL_BLACKLOTUS_STEAL_CASH_HACK")
        }
        HostCommandSpecialPowerType::BlackLotusCaptureBuilding => {
            Some("SPECIAL_BLACKLOTUS_CAPTURE_BUILDING")
        }
        HostCommandSpecialPowerType::RangerCaptureBuilding
        | HostCommandSpecialPowerType::RedGuardCaptureBuilding
        | HostCommandSpecialPowerType::RebelCaptureBuilding => {
            Some("SPECIAL_INFANTRY_CAPTURE_BUILDING")
        }
        HostCommandSpecialPowerType::DisguiseAsVehiclePower => Some("SPECIAL_DISGUISE_AS_VEHICLE"),
        HostCommandSpecialPowerType::Invalid => Some("SPECIAL_INVALID"),
        // FireWall is a host FIRE_WEAPON residual (Dragon Tank), not a SpecialPowerType
        // discriminant in C++ SpecialPowerType.h — no unique SPECIAL_* name residual.
        HostCommandSpecialPowerType::FireWall => None,
        // Host residual placeholders without unique C++ SPECIAL_* discriminant.
        HostCommandSpecialPowerType::Healing
        | HostCommandSpecialPowerType::IonCannon
        | HostCommandSpecialPowerType::SuperweaponCountermeasures => None,
    }
}

/// Lookup bit-name list index for a C++ SPECIAL_* residual string.
pub fn special_power_bit_name_index(name: &str) -> Option<usize> {
    SPECIAL_POWER_BIT_NAME_LIST.iter().position(|&n| n == name)
}

/// Wave 80 honesty: SpecialPower enum residual discriminants pack.
///
/// Fail-closed: not full SpecialPowerStore Xfer rebind / mask ops.

/// Residual SpecialPower ReloadTime in seconds for host cooldown consume path.
///
/// Fail-closed: unknown powers keep object template cooldown (caller default).

/// C++ `SpecialPowerTemplate::m_sharedNSync` / INI `SharedSyncedTimer` residual.
///
/// Superweapons shared across a player's command centers return `true`.
/// Unit special abilities (TNT, capture, disable, laser lock, charges) return
/// `false` and keep per-object timers only.
///
/// Fail-closed: not full SpecialPowerStore template ID binding / PublicTimer UI.

/// C++ `SpecialPowerTemplate::m_requiredScience` residual (SpecialPower.ini).
///
/// `None` = SCIENCE_INVALID / no RequiredScience line (always science-ok).
/// Tier-1 science name residual: higher tiers still satisfy via player unlock
/// of that same key or a higher tier when callers unlock them explicitly.
///
/// Fail-closed: not full ScienceStore purchase graph / multi-template aliases.
pub fn special_power_required_science(
    power: &crate::command_system::SpecialPowerType,
) -> Option<&'static str> {
    use crate::command_system::SpecialPowerType as P;
    match power {
        P::DaisyCutter | P::FuelAirBomb | P::AirForceDaisyCutter => Some("SCIENCE_DaisyCutter"),
        P::Airstrike => Some("SCIENCE_A10ThunderboltMissileStrike1"),
        P::AirForceAirstrike => Some("AirF_SCIENCE_A10ThunderboltMissileStrike1"),
        P::NapalmStrike => Some("SCIENCE_NapalmStrike"),
        P::BlackMarketNuke => Some("SCIENCE_BlackMarketNuke"),
        P::SpectreGunship => Some("SCIENCE_SpectreGunshipSolo"),
        P::AirForceSpectreGunship => Some("SCIENCE_SpectreGunship1"),
        // Public-timer carpet bomb has RequiredScience commented out in retail INI.
        P::CarpetBomb => None,
        P::AirForceCarpetBomb => Some("SCIENCE_AirF_CarpetBomb"),
        P::EarlyChinaCarpetBomb => Some("Early_SCIENCE_ChinaCarpetBomb"),
        P::NukeChinaCarpetBomb => Some("Nuke_SCIENCE_ChinaCarpetBomb"),
        P::Artillery => Some("SCIENCE_ArtilleryBarrage1"),
        P::ClusterMines => Some("SCIENCE_ClusterMines"),
        P::NukeDrop => Some("Nuke_SCIENCE_NukeDrop"),
        P::EmpPulse => Some("SCIENCE_EMPPulse"),
        P::Paradrop => Some("SCIENCE_Paradrop1"),
        P::InfantryParadrop => Some("Infa_SCIENCE_InfantryParadrop1"),
        P::TankParadrop => Some("SCIENCE_TankParadrop1"),
        P::Ambush => Some("SCIENCE_RebelAmbush1"),
        P::TerrorCell => Some("SCIENCE_TerrorCell"),
        P::LeafletDrop => Some("SCIENCE_LeafletDrop"),
        P::EarlyLeafletDrop => Some("Early_SCIENCE_LeafletDrop"),
        P::Frenzy => Some("SCIENCE_Frenzy1"),
        P::EarlyFrenzy => Some("Early_SCIENCE_Frenzy1"),
        P::EmergencyRepair => Some("SCIENCE_EmergencyRepair1"),
        P::EarlyEmergencyRepair => Some("Early_SCIENCE_EmergencyRepair1"),
        P::GpsScrambler => Some("SCIENCE_GPSScrambler"),
        P::StealthGpsScrambler => Some("Slth_SCIENCE_GPSScrambler"),
        P::CashHack => Some("SCIENCE_CashHack1"),
        P::CrateDrop => Some("SCIENCE_CrateDrop"),
        P::SneakAttack => Some("SCIENCE_SneakAttack"),
        P::SpyDrone => Some("SCIENCE_SpyDrone"),
        P::AnthraxBomb => Some("SCIENCE_AnthraxBomb"),
        // Structure-built / no RequiredScience residual.
        P::NuclearMissile
        | P::BaikonurRocket
        | P::NukeNeutronMissile
        | P::SuperweaponNeutronMissile
        | P::DetonateDirtyNuke
        | P::ParticleCannon
        | P::LaserCannon
        | P::SuperweaponParticleCannon
        | P::ScudStorm
        | P::SpySatellite
        | P::RadarScan
        | P::CiaIntelligence
        | P::CommunicationsDownload
        | P::BattleshipBombardment
        | P::CruiseMissile
        | P::FireWall
        | P::IonCannon => None,
        // Unit special abilities: no player science gate residual.
        _ => None,
    }
}

/// True when player science residual allows firing this power.
///
/// C++ `SpecialPowerStore::canUseSpecialPower` science branch residual.
pub fn player_meets_special_power_science(
    unlocked_sciences: &std::collections::HashSet<String>,
    power: &crate::command_system::SpecialPowerType,
) -> bool {
    let Some(required) = special_power_required_science(power) else {
        return true;
    };
    // Normalize like Player::has_unlocked_upgrade residual (case/underscore).
    let req = required.to_ascii_lowercase().replace('-', "_");
    unlocked_sciences.iter().any(|s| {
        let u = s.to_ascii_lowercase().replace('-', "_");
        u == req || u.ends_with(&req) || req.ends_with(&u)
    })
}

/// C++ SpecialPowerTemplate::hasPublicTimer residual (SpecialPower.ini PublicTimer=Yes).
///
/// These powers show on the InGameUI superweapon countdown list for the local player.
/// Fail-closed: not full InGameUI font flash / multi-object SW map / script hide.

/// Host structure template residual that provides a PublicTimer superweapon.
///
/// Used by presentation/InGameUI to unlock countdown rows when the local player
/// owns a living constructed superweapon building (C++ addSuperweapon path).
/// Fail-closed: not full multi-general template aliases / capture transfer matrix.
pub fn special_power_public_timer_structure_templates(
    power: &crate::command_system::SpecialPowerType,
) -> &'static [&'static str] {
    use crate::command_system::SpecialPowerType as P;
    use crate::game_logic::host_superweapon_kindof::{
        AMERICA_PARTICLE_CANNON_UPLINK, CHINA_NUCLEAR_MISSILE_LAUNCHER, GLA_SCUD_STORM,
    };
    match power {
        P::ParticleCannon | P::SuperweaponParticleCannon | P::LaserCannon => &[
            AMERICA_PARTICLE_CANNON_UPLINK,
            "AmericaParticleUplinkCannon",
            "SupW_AmericaParticleCannonUplink",
            "Lazr_AmericaParticleCannonUplink",
        ],
        P::NuclearMissile
        | P::NukeNeutronMissile
        | P::SuperweaponNeutronMissile
        | P::BaikonurRocket => &[
            CHINA_NUCLEAR_MISSILE_LAUNCHER,
            "Nuke_ChinaNuclearMissileLauncher",
            "SupW_AmericaNuclearMissile",
        ],
        P::ScudStorm => &[GLA_SCUD_STORM, "Chem_GLAScudStorm"],
        // Science-gated PublicTimer powers: no structure template residual.
        _ => &[],
    }
}

/// True when `template_name` matches a structure residual for this public timer power.
pub fn template_provides_public_timer_power(
    power: &crate::command_system::SpecialPowerType,
    template_name: &str,
) -> bool {
    let t = template_name.to_ascii_lowercase();
    special_power_public_timer_structure_templates(power)
        .iter()
        .any(|s| t == s.to_ascii_lowercase() || t.contains(&s.to_ascii_lowercase()))
}

pub fn special_power_has_public_timer(power: &crate::command_system::SpecialPowerType) -> bool {
    use crate::command_system::SpecialPowerType as P;
    matches!(
        power,
        P::CarpetBomb
            | P::CrateDrop
            | P::NapalmStrike
            | P::NuclearMissile
            | P::NukeNeutronMissile
            | P::SuperweaponNeutronMissile
            | P::BaikonurRocket
            | P::ScudStorm
            | P::TerrorCell
            | P::BlackMarketNuke
            | P::ParticleCannon
            | P::SuperweaponParticleCannon
            | P::LaserCannon
            | P::CruiseMissile
    )
}

/// Display name residual for public-timer HUD rows.
pub fn special_power_public_timer_display_name(
    power: &crate::command_system::SpecialPowerType,
) -> &'static str {
    use crate::command_system::SpecialPowerType as P;
    match power {
        P::CarpetBomb => "Carpet Bomb",
        P::CrateDrop => "Crate Drop",
        P::NapalmStrike => "Napalm Strike",
        P::NuclearMissile
        | P::NukeNeutronMissile
        | P::SuperweaponNeutronMissile
        | P::BaikonurRocket => "Nuclear Missile",
        P::ScudStorm => "Scud Storm",
        P::TerrorCell => "Terror Cell",
        P::BlackMarketNuke => "Black Market Nuke",
        P::ParticleCannon | P::SuperweaponParticleCannon | P::LaserCannon => "Particle Cannon",
        P::CruiseMissile => "Cruise Missile",
        _ => "Superweapon",
    }
}

/// Icon residual key for public-timer HUD (command-button style).
pub fn special_power_public_timer_icon(
    power: &crate::command_system::SpecialPowerType,
) -> &'static str {
    use crate::command_system::SpecialPowerType as P;
    match power {
        P::CarpetBomb => "SSCarpetBomb",
        P::CrateDrop => "SSCrateDrop",
        P::NapalmStrike => "SSNapalmStrike",
        P::NuclearMissile
        | P::NukeNeutronMissile
        | P::SuperweaponNeutronMissile
        | P::BaikonurRocket => "SSNuclearMissile",
        P::ScudStorm => "SSScudStorm",
        P::TerrorCell => "SSTerrorCell",
        P::BlackMarketNuke => "SSBlackMarketNuke",
        P::ParticleCannon | P::SuperweaponParticleCannon | P::LaserCannon => "SSParticleCannon",
        P::CruiseMissile => "SSCruiseMissile",
        _ => "SSSuperweapon",
    }
}

/// True for structure-bound PublicTimer superweapons that are **not** SharedNSync.
///
/// Retail: Particle Uplink / Neutron Missile / Scud Storm use PublicTimer=Yes with
/// no SharedNSync — ready frame is per-structure (C++ SpecialPowerModule).
pub fn special_power_is_structure_bound_public_timer(
    power: &crate::command_system::SpecialPowerType,
) -> bool {
    use crate::command_system::SpecialPowerType as P;
    matches!(
        power,
        P::ParticleCannon
            | P::SuperweaponParticleCannon
            | P::LaserCannon
            | P::NuclearMissile
            | P::NukeNeutronMissile
            | P::SuperweaponNeutronMissile
            | P::ScudStorm
    )
}

pub fn special_power_uses_shared_synced_timer(
    power: &crate::command_system::SpecialPowerType,
) -> bool {
    use crate::command_system::SpecialPowerType as P;
    match power {
        // Superweapons / player-shared residual (SharedSyncedTimer = Yes in SpecialPower.ini).
        P::DaisyCutter
        | P::FuelAirBomb
        | P::AirForceDaisyCutter
        | P::Airstrike
        | P::AirForceAirstrike
        | P::NapalmStrike
        // Structure PublicTimer SWs (PUC/Nuke/Scud) are NOT SharedNSync in retail
        // SpecialPower.ini — countdown lives on the building module, not Player.
        | P::BaikonurRocket
        | P::BlackMarketNuke
        | P::DetonateDirtyNuke
        | P::SpectreGunship
        | P::AirForceSpectreGunship
        | P::CarpetBomb
        | P::AirForceCarpetBomb
        | P::EarlyChinaCarpetBomb
        | P::NukeChinaCarpetBomb
        | P::AnthraxBomb
        | P::Artillery
        | P::BattleshipBombardment
        | P::CruiseMissile
        | P::ClusterMines
        | P::NukeDrop
        | P::EmpPulse
        | P::Paradrop
        | P::InfantryParadrop
        | P::TankParadrop
        | P::Ambush
        | P::TerrorCell
        | P::LeafletDrop
        | P::EarlyLeafletDrop
        | P::Frenzy
        | P::EarlyFrenzy
        | P::EmergencyRepair
        | P::EarlyEmergencyRepair
        | P::GpsScrambler
        | P::StealthGpsScrambler
        | P::CiaIntelligence
        | P::CommunicationsDownload
        | P::CashHack
        | P::CrateDrop
        | P::SneakAttack
        | P::SpySatellite
        | P::SpyDrone
        | P::RadarScan
        | P::FireWall
        | P::IonCannon => true,
        // Unit abilities / non-shared residual.
        P::HelixNapalmBomb
        | P::HelixNukeBomb
        | P::TankHunterTnt
        | P::MissileDefenderLaserGuided
        | P::LaserGuidedHowitzer
        | P::DemoRebelTimedCharges
        | P::BattleBusDemoTrapRollout
        | P::DemoKellTimedCharges
        | P::DemoKellStickyCharges
        | P::DemoKellRemoteCharges
        | P::BurtonTimedCharges
        | P::BurtonRemoteCharges
        | P::HackerDisableBuilding
        | P::MicrowaveDisableBuilding
        | P::BlackLotusDisableVehicle
        | P::BlackLotusStealCash
        | P::BlackLotusCaptureBuilding
        | P::RangerCaptureBuilding
        | P::RedGuardCaptureBuilding
        | P::RebelCaptureBuilding
        | P::DisguiseAsVehiclePower
        | P::CleanupArea
        | P::BattlePlanBombardment
        | P::BattlePlanHoldTheLine
        | P::BattlePlanSearchAndDestroy
        | P::Invalid => false,
        // Unknown residual: prefer shared for superweapon-like names is unsafe;
        // fail-closed to per-object so unit abilities never block teammates.
        _ => false,
    }
}

pub fn special_power_reload_seconds(
    power: &crate::command_system::SpecialPowerType,
) -> Option<f32> {
    use crate::command_system::SpecialPowerType as P;
    use crate::game_logic::host_ambush::AMBUSH_RELOAD_TIME_MS;
    use crate::game_logic::host_cia_intelligence::CIA_INTELLIGENCE_RELOAD_MS;
    use crate::game_logic::host_emergency_repair::EMERGENCY_REPAIR_RELOAD_TIME_MS;
    use crate::game_logic::host_frenzy::FRENZY_RELOAD_TIME_MS;
    use crate::game_logic::host_gps_scrambler::GPS_SCRAMBLER_RELOAD_MS;
    use crate::game_logic::host_helix_napalm::HELIX_NAPALM_RELOAD_MS;
    use crate::game_logic::host_leaflet_drop::LEAFLET_RELOAD_MS;
    use crate::game_logic::host_missile_defender::LASER_GUIDED_RELOAD_MS;
    use crate::game_logic::host_paradrop::PARADROP_RELOAD_MS;
    use crate::game_logic::host_sneak_attack::SNEAK_ATTACK_RELOAD_TIME_MS;
    use crate::game_logic::host_tank_hunter::TNT_RELOAD_MS;
    use crate::game_logic::special_power_strikes::{
        A10_STRIKE_RELOAD_MS, AIRF_CARPET_RELOAD_MS, BLACK_MARKET_NUKE_RELOAD_MS,
        CARPET_BOMB_RELOAD_MS, DAISY_CUTTER_RELOAD_MS, DIRTY_NUKE_RELOAD_MS,
        EARLY_CHINA_CARPET_RELOAD_MS, NAPALM_STRIKE_RELOAD_MS, NUCLEAR_MISSILE_RELOAD_MS,
        SPECTRE_AIRF_RELOAD_MS, SPECTRE_RELOAD_MS,
    };

    let ms: Option<u32> = match power {
        P::DaisyCutter | P::FuelAirBomb | P::AirForceDaisyCutter => Some(DAISY_CUTTER_RELOAD_MS),
        P::Airstrike | P::AirForceAirstrike => Some(A10_STRIKE_RELOAD_MS),
        P::NapalmStrike => Some(NAPALM_STRIKE_RELOAD_MS),
        P::NuclearMissile | P::BaikonurRocket => Some(NUCLEAR_MISSILE_RELOAD_MS),
        P::BlackMarketNuke => Some(BLACK_MARKET_NUKE_RELOAD_MS),
        P::DetonateDirtyNuke => Some(DIRTY_NUKE_RELOAD_MS),
        P::NukeNeutronMissile => Some(300_000),
        P::SuperweaponNeutronMissile => Some(240_000),
        P::SpectreGunship => Some(SPECTRE_RELOAD_MS),
        P::AirForceSpectreGunship => Some(SPECTRE_AIRF_RELOAD_MS),
        P::CarpetBomb => Some(CARPET_BOMB_RELOAD_MS),
        P::AirForceCarpetBomb => Some(AIRF_CARPET_RELOAD_MS),
        P::EarlyChinaCarpetBomb | P::NukeChinaCarpetBomb => Some(EARLY_CHINA_CARPET_RELOAD_MS),
        P::ParticleCannon | P::LaserCannon => Some(240_000),
        P::SuperweaponParticleCannon => Some(180_000),
        P::ScudStorm => Some(300_000),
        P::AnthraxBomb => Some(360_000),
        P::Artillery | P::BattleshipBombardment => Some(240_000),
        P::CruiseMissile => Some(120_000),
        P::ClusterMines | P::NukeDrop => Some(240_000),
        P::EmpPulse => Some(240_000),
        P::Paradrop | P::InfantryParadrop | P::TankParadrop => Some(PARADROP_RELOAD_MS),
        P::Ambush | P::TerrorCell => Some(AMBUSH_RELOAD_TIME_MS),
        P::LeafletDrop | P::EarlyLeafletDrop => Some(LEAFLET_RELOAD_MS),
        P::Frenzy | P::EarlyFrenzy => Some(FRENZY_RELOAD_TIME_MS),
        P::EmergencyRepair | P::EarlyEmergencyRepair => Some(EMERGENCY_REPAIR_RELOAD_TIME_MS),
        P::GpsScrambler | P::StealthGpsScrambler => Some(GPS_SCRAMBLER_RELOAD_MS),
        P::CiaIntelligence => Some(CIA_INTELLIGENCE_RELOAD_MS),
        P::CommunicationsDownload => Some(10_000),
        P::CashHack => Some(240_000),
        P::CrateDrop => Some(600_000),
        P::SneakAttack => Some(SNEAK_ATTACK_RELOAD_TIME_MS),
        P::HelixNapalmBomb | P::HelixNukeBomb => Some(HELIX_NAPALM_RELOAD_MS),
        P::TankHunterTnt => Some(TNT_RELOAD_MS),
        P::MissileDefenderLaserGuided | P::LaserGuidedHowitzer => Some(LASER_GUIDED_RELOAD_MS),
        P::DemoRebelTimedCharges => Some(30_000),
        P::BattleBusDemoTrapRollout => Some(7_500),
        P::DemoKellTimedCharges
        | P::DemoKellStickyCharges
        | P::DemoKellRemoteCharges
        | P::BurtonTimedCharges
        | P::BurtonRemoteCharges => Some(0),
        // Unit ability specials with short/no shared SW timer residual.
        P::HackerDisableBuilding => Some(500),
        P::MicrowaveDisableBuilding => Some(4_000),
        P::BlackLotusDisableVehicle => Some(0),
        P::BlackLotusStealCash => Some(2_000),
        P::BlackLotusCaptureBuilding => Some(0),
        P::RangerCaptureBuilding
        | P::RedGuardCaptureBuilding
        | P::RebelCaptureBuilding
        | P::DisguiseAsVehiclePower => Some(0),
        P::SpySatellite => Some(60_000),
        P::SpyDrone => Some(crate::game_logic::host_spy_drone::SPY_DRONE_RELOAD_MS),
        P::RadarScan => Some(60_000),
        P::CleanupArea => Some(0),
        P::BattlePlanBombardment | P::BattlePlanHoldTheLine | P::BattlePlanSearchAndDestroy => {
            Some(0)
        }
        _ => None,
    };
    ms.map(|m| m as f32 / 1000.0)
}

pub fn honesty_special_power_enum_residual_pack_wave80() -> bool {
    SPECIALPOWER_COUNT == 67
        && SPECIAL_POWER_BIT_NAME_LIST.len() == 67
        && SPECIAL_POWER_BIT_NAME_LIST[0] == "SPECIAL_INVALID"
        && SPECIAL_POWER_BIT_NAME_LIST[1] == "SPECIAL_DAISY_CUTTER"
        && SPECIAL_POWER_BIT_NAME_LIST[18] == "SPECIAL_A10_THUNDERBOLT_STRIKE"
        && SPECIAL_POWER_BIT_NAME_LIST[36] == "SPECIAL_PARTICLE_UPLINK_CANNON"
        && SPECIAL_POWER_BIT_NAME_LIST[42] == "SPECIAL_SPECTRE_GUNSHIP"
        && SPECIAL_POWER_BIT_NAME_LIST[63] == "SUPR_SPECIAL_CRUISE_MISSILE"
        && SPECIAL_POWER_BIT_NAME_LIST[66] == "SPECIAL_BATTLESHIP_BOMBARDMENT"
        && {
            use crate::command_system::SpecialPowerType as P;
            special_power_is_structure_bound_public_timer(&P::ParticleCannon)
                && special_power_is_structure_bound_public_timer(&P::ScudStorm)
                && special_power_is_structure_bound_public_timer(&P::NuclearMissile)
                && !special_power_uses_shared_synced_timer(&P::ParticleCannon)
                && !special_power_uses_shared_synced_timer(&P::ScudStorm)
                && !special_power_uses_shared_synced_timer(&P::NuclearMissile)
                && special_power_uses_shared_synced_timer(&P::DaisyCutter)
                && special_power_uses_shared_synced_timer(&P::Airstrike)
        }
        // Host superweapon ordinal ↔ name table residual.
        && [
            HostSuperweaponKind::DaisyCutter,
            HostSuperweaponKind::A10Strike,
            HostSuperweaponKind::ScudStorm,
            HostSuperweaponKind::ParticleCannon,
            HostSuperweaponKind::NuclearMissile,
            HostSuperweaponKind::AnthraxBomb,
            HostSuperweaponKind::SpectreGunship,
            HostSuperweaponKind::CarpetBomb,
            HostSuperweaponKind::ArtilleryBarrage,
            HostSuperweaponKind::CruiseMissile,
        ]
        .iter()
        .all(|k| {
            let name = k.cpp_special_power_enum_name();
            let ord = k.cpp_special_power_ordinal() as usize;
            SPECIAL_POWER_BIT_NAME_LIST.get(ord) == Some(&name)
                && special_power_bit_name_index(name) == Some(ord)
        })
        // Host command power bridge residual (sample of superweapons).
        && host_command_power_cpp_enum_name(&HostCommandSpecialPowerType::DaisyCutter)
            == Some("SPECIAL_DAISY_CUTTER")
        && host_command_power_cpp_enum_name(&HostCommandSpecialPowerType::Airstrike)
            == Some("SPECIAL_A10_THUNDERBOLT_STRIKE")
        && host_command_power_cpp_enum_name(&HostCommandSpecialPowerType::ParticleCannon)
            == Some("SPECIAL_PARTICLE_UPLINK_CANNON")
        && host_command_power_cpp_enum_name(&HostCommandSpecialPowerType::CruiseMissile)
            == Some("SUPR_SPECIAL_CRUISE_MISSILE")
        && host_command_power_cpp_enum_name(&HostCommandSpecialPowerType::Invalid)
            == Some("SPECIAL_INVALID")
        && host_command_power_cpp_enum_name(&HostCommandSpecialPowerType::IonCannon).is_none()
        // Unique names in table.
        && {
            let mut names: Vec<&str> = SPECIAL_POWER_BIT_NAME_LIST.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_power_enum_residual_pack_wave80_honesty() {
        assert!(honesty_special_power_enum_residual_pack_wave80());
        assert_eq!(
            HostSuperweaponKind::CruiseMissile.cpp_special_power_enum_name(),
            "SUPR_SPECIAL_CRUISE_MISSILE"
        );
        assert_eq!(
            HostSuperweaponKind::CruiseMissile.cpp_special_power_ordinal(),
            63
        );
    }
}
