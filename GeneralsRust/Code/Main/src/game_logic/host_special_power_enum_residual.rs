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
        HostCommandSpecialPowerType::EmergencyRepair => Some("SPECIAL_REPAIR_VEHICLES"),
        HostCommandSpecialPowerType::EarlyEmergencyRepair => Some("EARLY_SPECIAL_REPAIR_VEHICLES"),
        HostCommandSpecialPowerType::NapalmStrike => Some("SPECIAL_NAPALM_STRIKE"),
        HostCommandSpecialPowerType::NuclearMissile => Some("SPECIAL_NEUTRON_MISSILE"),
        HostCommandSpecialPowerType::BlackMarketNuke => Some("SPECIAL_BLACK_MARKET_NUKE"),
        HostCommandSpecialPowerType::DetonateDirtyNuke => Some("SPECIAL_DETONATE_DIRTY_NUKE"),
        HostCommandSpecialPowerType::Paradrop => Some("SPECIAL_PARADROP_AMERICA"),
        HostCommandSpecialPowerType::Ambush => Some("SPECIAL_AMBUSH"),
        HostCommandSpecialPowerType::ParticleCannon => Some("SPECIAL_PARTICLE_UPLINK_CANNON"),
        HostCommandSpecialPowerType::RadarScan => Some("SPECIAL_RADAR_VAN_SCAN"),
        HostCommandSpecialPowerType::ScudStorm => Some("SPECIAL_SCUD_STORM"),
        HostCommandSpecialPowerType::SpySatellite => Some("SPECIAL_SPY_SATELLITE"),
        HostCommandSpecialPowerType::CiaIntelligence => Some("SPECIAL_CIA_INTELLIGENCE"),
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
        HostCommandSpecialPowerType::CashHack => Some("SPECIAL_CASH_HACK"),
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
