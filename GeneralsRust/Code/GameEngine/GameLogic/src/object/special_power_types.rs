// FILE: special_power_types.rs
// Port of SpecialPowerType.h and SpecialPowerMaskType.h
// Author: Rust Port
// Desc: Special power type enumerations and bit mask types for Generals abilities

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

/// Special power type enumeration
/// Note: These values are saved in save files, so you MUST NOT REMOVE OR CHANGE existing values!
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum SpecialPowerType {
    Invalid = 0,

    // Superweapons
    DaisyCutter = 1,
    ParadropAmerica = 2,
    CarpetBomb = 3,
    ClusterMines = 4,
    EmpPulse = 5,
    NapalmStrike = 6,
    CashHack = 7,
    NeutronMissile = 8,
    SpySatellite = 9,
    Defector = 10,
    TerrorCell = 11,
    Ambush = 12,
    BlackMarketNuke = 13,
    AnthraxBomb = 14,
    ScudStorm = 15,
    Demoralize = 16,
    CrateDrop = 17,
    A10ThunderboltStrike = 18,
    DetonateDirtyNuke = 19,
    ArtilleryBarrage = 20,

    // Special abilities
    MissileDefenderLaserGuidedMissiles = 21,
    RemoteCharges = 22,
    TimedCharges = 23,
    HelixNapalmBomb = 24,
    HackerDisableBuilding = 25,
    TankHunterTntAttack = 26,
    BlackLotusCaptureBuilding = 27,
    BlackLotusDisableVehicleHack = 28,
    BlackLotusStealCashHack = 29,
    InfantryCaptureBuilding = 30,
    RadarVanScan = 31,
    SpyDrone = 32,
    DisguiseAsVehicle = 33,
    BoobyTrap = 34,
    RepairVehicles = 35,
    ParticleUplinkCannon = 36,
    CashBounty = 37,
    ChangeBattlePlans = 38,
    CiaIntelligence = 39,
    CleanupArea = 40,
    LaunchBaikonurRocket = 41,
    SpectreGunship = 42,
    GpsScrambler = 43,
    Frenzy = 44,
    SneakAttack = 45,

    // Additional enums for faction-specific variants
    ChinaCarpetBomb = 46,
    EarlyChinaCarpetBomb = 47,
    LeafletDrop = 48,
    EarlyLeafletDrop = 49,
    EarlyFrenzy = 50,
    CommunicationsDownload = 51,
    EarlyRepairVehicles = 52,
    TankParadrop = 53,
    SupwParticleUplinkCannon = 54,
    AirfDaisyCutter = 55,
    NukeClusterMines = 56,
    NukeNeutronMissile = 57,
    AirfA10ThunderboltStrike = 58,
    AirfSpectreGunship = 59,
    InfaParadropAmerica = 60,
    SlthGpsScrambler = 61,
    AirfCarpetBomb = 62,
    SuprCruiseMissile = 63,
    LazrParticleUplinkCannon = 64,
    SupwNeutronMissile = 65,
    BattleshipBombardment = 66,

    Count = 67,
}

impl Default for SpecialPowerType {
    fn default() -> Self {
        SpecialPowerType::Invalid
    }
}

impl SpecialPowerType {
    /// Get the special power type name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            SpecialPowerType::Invalid => "SPECIAL_INVALID",
            SpecialPowerType::DaisyCutter => "SPECIAL_DAISY_CUTTER",
            SpecialPowerType::ParadropAmerica => "SPECIAL_PARADROP_AMERICA",
            SpecialPowerType::CarpetBomb => "SPECIAL_CARPET_BOMB",
            SpecialPowerType::ClusterMines => "SPECIAL_CLUSTER_MINES",
            SpecialPowerType::EmpPulse => "SPECIAL_EMP_PULSE",
            SpecialPowerType::NapalmStrike => "SPECIAL_NAPALM_STRIKE",
            SpecialPowerType::CashHack => "SPECIAL_CASH_HACK",
            SpecialPowerType::NeutronMissile => "SPECIAL_NEUTRON_MISSILE",
            SpecialPowerType::SpySatellite => "SPECIAL_SPY_SATELLITE",
            SpecialPowerType::Defector => "SPECIAL_DEFECTOR",
            SpecialPowerType::TerrorCell => "SPECIAL_TERROR_CELL",
            SpecialPowerType::Ambush => "SPECIAL_AMBUSH",
            SpecialPowerType::BlackMarketNuke => "SPECIAL_BLACK_MARKET_NUKE",
            SpecialPowerType::AnthraxBomb => "SPECIAL_ANTHRAX_BOMB",
            SpecialPowerType::ScudStorm => "SPECIAL_SCUD_STORM",
            SpecialPowerType::Demoralize => "SPECIAL_DEMORALIZE",
            SpecialPowerType::CrateDrop => "SPECIAL_CRATE_DROP",
            SpecialPowerType::A10ThunderboltStrike => "SPECIAL_A10_THUNDERBOLT_STRIKE",
            SpecialPowerType::DetonateDirtyNuke => "SPECIAL_DETONATE_DIRTY_NUKE",
            SpecialPowerType::ArtilleryBarrage => "SPECIAL_ARTILLERY_BARRAGE",
            SpecialPowerType::MissileDefenderLaserGuidedMissiles => {
                "SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES"
            }
            SpecialPowerType::RemoteCharges => "SPECIAL_REMOTE_CHARGES",
            SpecialPowerType::TimedCharges => "SPECIAL_TIMED_CHARGES",
            SpecialPowerType::HelixNapalmBomb => "SPECIAL_HELIX_NAPALM_BOMB",
            SpecialPowerType::HackerDisableBuilding => "SPECIAL_HACKER_DISABLE_BUILDING",
            SpecialPowerType::TankHunterTntAttack => "SPECIAL_TANKHUNTER_TNT_ATTACK",
            SpecialPowerType::BlackLotusCaptureBuilding => "SPECIAL_BLACKLOTUS_CAPTURE_BUILDING",
            SpecialPowerType::BlackLotusDisableVehicleHack => {
                "SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK"
            }
            SpecialPowerType::BlackLotusStealCashHack => "SPECIAL_BLACKLOTUS_STEAL_CASH_HACK",
            SpecialPowerType::InfantryCaptureBuilding => "SPECIAL_INFANTRY_CAPTURE_BUILDING",
            SpecialPowerType::RadarVanScan => "SPECIAL_RADAR_VAN_SCAN",
            SpecialPowerType::SpyDrone => "SPECIAL_SPY_DRONE",
            SpecialPowerType::DisguiseAsVehicle => "SPECIAL_DISGUISE_AS_VEHICLE",
            SpecialPowerType::BoobyTrap => "SPECIAL_BOOBY_TRAP",
            SpecialPowerType::RepairVehicles => "SPECIAL_REPAIR_VEHICLES",
            SpecialPowerType::ParticleUplinkCannon => "SPECIAL_PARTICLE_UPLINK_CANNON",
            SpecialPowerType::CashBounty => "SPECIAL_CASH_BOUNTY",
            SpecialPowerType::ChangeBattlePlans => "SPECIAL_CHANGE_BATTLE_PLANS",
            SpecialPowerType::CiaIntelligence => "SPECIAL_CIA_INTELLIGENCE",
            SpecialPowerType::CleanupArea => "SPECIAL_CLEANUP_AREA",
            SpecialPowerType::LaunchBaikonurRocket => "SPECIAL_LAUNCH_BAIKONUR_ROCKET",
            SpecialPowerType::SpectreGunship => "SPECIAL_SPECTRE_GUNSHIP",
            SpecialPowerType::GpsScrambler => "SPECIAL_GPS_SCRAMBLER",
            SpecialPowerType::Frenzy => "SPECIAL_FRENZY",
            SpecialPowerType::SneakAttack => "SPECIAL_SNEAK_ATTACK",
            SpecialPowerType::ChinaCarpetBomb => "SPECIAL_CHINA_CARPET_BOMB",
            SpecialPowerType::EarlyChinaCarpetBomb => "EARLY_SPECIAL_CHINA_CARPET_BOMB",
            SpecialPowerType::LeafletDrop => "SPECIAL_LEAFLET_DROP",
            SpecialPowerType::EarlyLeafletDrop => "EARLY_SPECIAL_LEAFLET_DROP",
            SpecialPowerType::EarlyFrenzy => "EARLY_SPECIAL_FRENZY",
            SpecialPowerType::CommunicationsDownload => "SPECIAL_COMMUNICATIONS_DOWNLOAD",
            SpecialPowerType::EarlyRepairVehicles => "EARLY_SPECIAL_REPAIR_VEHICLES",
            SpecialPowerType::TankParadrop => "SPECIAL_TANK_PARADROP",
            SpecialPowerType::SupwParticleUplinkCannon => "SUPW_SPECIAL_PARTICLE_UPLINK_CANNON",
            SpecialPowerType::AirfDaisyCutter => "AIRF_SPECIAL_DAISY_CUTTER",
            SpecialPowerType::NukeClusterMines => "NUKE_SPECIAL_CLUSTER_MINES",
            SpecialPowerType::NukeNeutronMissile => "NUKE_SPECIAL_NEUTRON_MISSILE",
            SpecialPowerType::AirfA10ThunderboltStrike => "AIRF_SPECIAL_A10_THUNDERBOLT_STRIKE",
            SpecialPowerType::AirfSpectreGunship => "AIRF_SPECIAL_SPECTRE_GUNSHIP",
            SpecialPowerType::InfaParadropAmerica => "INFA_SPECIAL_PARADROP_AMERICA",
            SpecialPowerType::SlthGpsScrambler => "SLTH_SPECIAL_GPS_SCRAMBLER",
            SpecialPowerType::AirfCarpetBomb => "AIRF_SPECIAL_CARPET_BOMB",
            SpecialPowerType::SuprCruiseMissile => "SUPR_SPECIAL_CRUISE_MISSILE",
            SpecialPowerType::LazrParticleUplinkCannon => "LAZR_SPECIAL_PARTICLE_UPLINK_CANNON",
            SpecialPowerType::SupwNeutronMissile => "SUPW_SPECIAL_NEUTRON_MISSILE",
            SpecialPowerType::BattleshipBombardment => "SPECIAL_BATTLESHIP_BOMBARDMENT",
            SpecialPowerType::Count => "SPECIALPOWER_COUNT",
        }
    }

    /// Parse a special power type from a string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "SPECIAL_INVALID" => Some(SpecialPowerType::Invalid),
            "SPECIAL_DAISY_CUTTER" => Some(SpecialPowerType::DaisyCutter),
            "SPECIAL_PARADROP_AMERICA" => Some(SpecialPowerType::ParadropAmerica),
            "SPECIAL_CARPET_BOMB" => Some(SpecialPowerType::CarpetBomb),
            "SPECIAL_CLUSTER_MINES" => Some(SpecialPowerType::ClusterMines),
            "SPECIAL_EMP_PULSE" => Some(SpecialPowerType::EmpPulse),
            "SPECIAL_NAPALM_STRIKE" => Some(SpecialPowerType::NapalmStrike),
            "SPECIAL_CASH_HACK" => Some(SpecialPowerType::CashHack),
            "SPECIAL_NEUTRON_MISSILE" => Some(SpecialPowerType::NeutronMissile),
            "SPECIAL_SPY_SATELLITE" => Some(SpecialPowerType::SpySatellite),
            "SPECIAL_DEFECTOR" => Some(SpecialPowerType::Defector),
            "SPECIAL_TERROR_CELL" => Some(SpecialPowerType::TerrorCell),
            "SPECIAL_AMBUSH" => Some(SpecialPowerType::Ambush),
            "SPECIAL_BLACK_MARKET_NUKE" => Some(SpecialPowerType::BlackMarketNuke),
            "SPECIAL_ANTHRAX_BOMB" => Some(SpecialPowerType::AnthraxBomb),
            "SPECIAL_SCUD_STORM" => Some(SpecialPowerType::ScudStorm),
            "SPECIAL_DEMORALIZE" => Some(SpecialPowerType::Demoralize),
            "SPECIAL_CRATE_DROP" => Some(SpecialPowerType::CrateDrop),
            "SPECIAL_A10_THUNDERBOLT_STRIKE" => Some(SpecialPowerType::A10ThunderboltStrike),
            "SPECIAL_DETONATE_DIRTY_NUKE" => Some(SpecialPowerType::DetonateDirtyNuke),
            "SPECIAL_ARTILLERY_BARRAGE" => Some(SpecialPowerType::ArtilleryBarrage),
            "SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES" => {
                Some(SpecialPowerType::MissileDefenderLaserGuidedMissiles)
            }
            "SPECIAL_REMOTE_CHARGES" => Some(SpecialPowerType::RemoteCharges),
            "SPECIAL_TIMED_CHARGES" => Some(SpecialPowerType::TimedCharges),
            "SPECIAL_HELIX_NAPALM_BOMB" => Some(SpecialPowerType::HelixNapalmBomb),
            "SPECIAL_HACKER_DISABLE_BUILDING" => Some(SpecialPowerType::HackerDisableBuilding),
            "SPECIAL_TANKHUNTER_TNT_ATTACK" => Some(SpecialPowerType::TankHunterTntAttack),
            "SPECIAL_BLACKLOTUS_CAPTURE_BUILDING" => {
                Some(SpecialPowerType::BlackLotusCaptureBuilding)
            }
            "SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK" => {
                Some(SpecialPowerType::BlackLotusDisableVehicleHack)
            }
            "SPECIAL_BLACKLOTUS_STEAL_CASH_HACK" => Some(SpecialPowerType::BlackLotusStealCashHack),
            "SPECIAL_INFANTRY_CAPTURE_BUILDING" => Some(SpecialPowerType::InfantryCaptureBuilding),
            "SPECIAL_RADAR_VAN_SCAN" => Some(SpecialPowerType::RadarVanScan),
            "SPECIAL_SPY_DRONE" => Some(SpecialPowerType::SpyDrone),
            "SPECIAL_DISGUISE_AS_VEHICLE" => Some(SpecialPowerType::DisguiseAsVehicle),
            "SPECIAL_BOOBY_TRAP" => Some(SpecialPowerType::BoobyTrap),
            "SPECIAL_REPAIR_VEHICLES" => Some(SpecialPowerType::RepairVehicles),
            "SPECIAL_PARTICLE_UPLINK_CANNON" => Some(SpecialPowerType::ParticleUplinkCannon),
            "SPECIAL_CASH_BOUNTY" => Some(SpecialPowerType::CashBounty),
            "SPECIAL_CHANGE_BATTLE_PLANS" => Some(SpecialPowerType::ChangeBattlePlans),
            "SPECIAL_CIA_INTELLIGENCE" => Some(SpecialPowerType::CiaIntelligence),
            "SPECIAL_CLEANUP_AREA" => Some(SpecialPowerType::CleanupArea),
            "SPECIAL_LAUNCH_BAIKONUR_ROCKET" => Some(SpecialPowerType::LaunchBaikonurRocket),
            "SPECIAL_SPECTRE_GUNSHIP" => Some(SpecialPowerType::SpectreGunship),
            "SPECIAL_GPS_SCRAMBLER" => Some(SpecialPowerType::GpsScrambler),
            "SPECIAL_FRENZY" => Some(SpecialPowerType::Frenzy),
            "SPECIAL_SNEAK_ATTACK" => Some(SpecialPowerType::SneakAttack),
            "SPECIAL_CHINA_CARPET_BOMB" => Some(SpecialPowerType::ChinaCarpetBomb),
            "EARLY_SPECIAL_CHINA_CARPET_BOMB" => Some(SpecialPowerType::EarlyChinaCarpetBomb),
            "SPECIAL_LEAFLET_DROP" => Some(SpecialPowerType::LeafletDrop),
            "EARLY_SPECIAL_LEAFLET_DROP" => Some(SpecialPowerType::EarlyLeafletDrop),
            "EARLY_SPECIAL_FRENZY" => Some(SpecialPowerType::EarlyFrenzy),
            "SPECIAL_COMMUNICATIONS_DOWNLOAD" => Some(SpecialPowerType::CommunicationsDownload),
            "EARLY_SPECIAL_REPAIR_VEHICLES" => Some(SpecialPowerType::EarlyRepairVehicles),
            "SPECIAL_TANK_PARADROP" => Some(SpecialPowerType::TankParadrop),
            "SUPW_SPECIAL_PARTICLE_UPLINK_CANNON" => {
                Some(SpecialPowerType::SupwParticleUplinkCannon)
            }
            "AIRF_SPECIAL_DAISY_CUTTER" => Some(SpecialPowerType::AirfDaisyCutter),
            "NUKE_SPECIAL_CLUSTER_MINES" => Some(SpecialPowerType::NukeClusterMines),
            "NUKE_SPECIAL_NEUTRON_MISSILE" => Some(SpecialPowerType::NukeNeutronMissile),
            "AIRF_SPECIAL_A10_THUNDERBOLT_STRIKE" => {
                Some(SpecialPowerType::AirfA10ThunderboltStrike)
            }
            "AIRF_SPECIAL_SPECTRE_GUNSHIP" => Some(SpecialPowerType::AirfSpectreGunship),
            "INFA_SPECIAL_PARADROP_AMERICA" => Some(SpecialPowerType::InfaParadropAmerica),
            "SLTH_SPECIAL_GPS_SCRAMBLER" => Some(SpecialPowerType::SlthGpsScrambler),
            "AIRF_SPECIAL_CARPET_BOMB" => Some(SpecialPowerType::AirfCarpetBomb),
            "SUPR_SPECIAL_CRUISE_MISSILE" => Some(SpecialPowerType::SuprCruiseMissile),
            "LAZR_SPECIAL_PARTICLE_UPLINK_CANNON" => {
                Some(SpecialPowerType::LazrParticleUplinkCannon)
            }
            "SUPW_SPECIAL_NEUTRON_MISSILE" => Some(SpecialPowerType::SupwNeutronMissile),
            "SPECIAL_BATTLESHIP_BOMBARDMENT" => Some(SpecialPowerType::BattleshipBombardment),
            _ => None,
        }
    }
}

bitflags! {
    /// Special power mask type for bit flag operations
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SpecialPowerMask: u128 {
        const DAISY_CUTTER = 1 << 1;
        const PARADROP_AMERICA = 1 << 2;
        const CARPET_BOMB = 1 << 3;
        const CLUSTER_MINES = 1 << 4;
        const EMP_PULSE = 1 << 5;
        const NAPALM_STRIKE = 1 << 6;
        const CASH_HACK = 1 << 7;
        const NEUTRON_MISSILE = 1 << 8;
        const SPY_SATELLITE = 1 << 9;
        const DEFECTOR = 1 << 10;
        const TERROR_CELL = 1 << 11;
        const AMBUSH = 1 << 12;
        const BLACK_MARKET_NUKE = 1 << 13;
        const ANTHRAX_BOMB = 1 << 14;
        const SCUD_STORM = 1 << 15;
        const DEMORALIZE = 1 << 16;
        const CRATE_DROP = 1 << 17;
        const A10_THUNDERBOLT_STRIKE = 1 << 18;
        const DETONATE_DIRTY_NUKE = 1 << 19;
        const ARTILLERY_BARRAGE = 1 << 20;
        const MISSILE_DEFENDER_LASER_GUIDED_MISSILES = 1 << 21;
        const REMOTE_CHARGES = 1 << 22;
        const TIMED_CHARGES = 1 << 23;
        const HELIX_NAPALM_BOMB = 1 << 24;
        const HACKER_DISABLE_BUILDING = 1 << 25;
        const TANKHUNTER_TNT_ATTACK = 1 << 26;
        const BLACKLOTUS_CAPTURE_BUILDING = 1 << 27;
        const BLACKLOTUS_DISABLE_VEHICLE_HACK = 1 << 28;
        const BLACKLOTUS_STEAL_CASH_HACK = 1 << 29;
        const INFANTRY_CAPTURE_BUILDING = 1 << 30;
        const RADAR_VAN_SCAN = 1 << 31;
        const SPY_DRONE = 1 << 32;
        const DISGUISE_AS_VEHICLE = 1 << 33;
        const BOOBY_TRAP = 1 << 34;
        const REPAIR_VEHICLES = 1 << 35;
        const PARTICLE_UPLINK_CANNON = 1 << 36;
        const CASH_BOUNTY = 1 << 37;
        const CHANGE_BATTLE_PLANS = 1 << 38;
        const CIA_INTELLIGENCE = 1 << 39;
        const CLEANUP_AREA = 1 << 40;
        const LAUNCH_BAIKONUR_ROCKET = 1 << 41;
        const SPECTRE_GUNSHIP = 1 << 42;
        const GPS_SCRAMBLER = 1 << 43;
        const FRENZY = 1 << 44;
        const SNEAK_ATTACK = 1 << 45;
    }
}

impl Default for SpecialPowerMask {
    fn default() -> Self {
        SpecialPowerMask::empty()
    }
}

impl SpecialPowerMask {
    /// Test if a specific special power type is set
    pub fn test_power(&self, power_type: SpecialPowerType) -> bool {
        match power_type {
            SpecialPowerType::Invalid => false,
            SpecialPowerType::DaisyCutter => self.contains(SpecialPowerMask::DAISY_CUTTER),
            SpecialPowerType::ParadropAmerica => self.contains(SpecialPowerMask::PARADROP_AMERICA),
            SpecialPowerType::CarpetBomb => self.contains(SpecialPowerMask::CARPET_BOMB),
            SpecialPowerType::ClusterMines => self.contains(SpecialPowerMask::CLUSTER_MINES),
            SpecialPowerType::EmpPulse => self.contains(SpecialPowerMask::EMP_PULSE),
            SpecialPowerType::NapalmStrike => self.contains(SpecialPowerMask::NAPALM_STRIKE),
            SpecialPowerType::CashHack => self.contains(SpecialPowerMask::CASH_HACK),
            SpecialPowerType::NeutronMissile => self.contains(SpecialPowerMask::NEUTRON_MISSILE),
            SpecialPowerType::SpySatellite => self.contains(SpecialPowerMask::SPY_SATELLITE),
            SpecialPowerType::Defector => self.contains(SpecialPowerMask::DEFECTOR),
            SpecialPowerType::TerrorCell => self.contains(SpecialPowerMask::TERROR_CELL),
            SpecialPowerType::Ambush => self.contains(SpecialPowerMask::AMBUSH),
            SpecialPowerType::BlackMarketNuke => self.contains(SpecialPowerMask::BLACK_MARKET_NUKE),
            SpecialPowerType::AnthraxBomb => self.contains(SpecialPowerMask::ANTHRAX_BOMB),
            SpecialPowerType::ScudStorm => self.contains(SpecialPowerMask::SCUD_STORM),
            SpecialPowerType::Demoralize => self.contains(SpecialPowerMask::DEMORALIZE),
            SpecialPowerType::CrateDrop => self.contains(SpecialPowerMask::CRATE_DROP),
            SpecialPowerType::A10ThunderboltStrike => {
                self.contains(SpecialPowerMask::A10_THUNDERBOLT_STRIKE)
            }
            SpecialPowerType::DetonateDirtyNuke => {
                self.contains(SpecialPowerMask::DETONATE_DIRTY_NUKE)
            }
            SpecialPowerType::ArtilleryBarrage => {
                self.contains(SpecialPowerMask::ARTILLERY_BARRAGE)
            }
            _ => false, // Other types not in mask
        }
    }

    /// Set a specific special power type
    pub fn set_power(&mut self, power_type: SpecialPowerType, value: bool) {
        match power_type {
            SpecialPowerType::Invalid => {}
            SpecialPowerType::DaisyCutter => self.set(SpecialPowerMask::DAISY_CUTTER, value),
            SpecialPowerType::ParadropAmerica => {
                self.set(SpecialPowerMask::PARADROP_AMERICA, value)
            }
            SpecialPowerType::CarpetBomb => self.set(SpecialPowerMask::CARPET_BOMB, value),
            SpecialPowerType::ClusterMines => self.set(SpecialPowerMask::CLUSTER_MINES, value),
            SpecialPowerType::EmpPulse => self.set(SpecialPowerMask::EMP_PULSE, value),
            SpecialPowerType::NapalmStrike => self.set(SpecialPowerMask::NAPALM_STRIKE, value),
            SpecialPowerType::CashHack => self.set(SpecialPowerMask::CASH_HACK, value),
            SpecialPowerType::NeutronMissile => self.set(SpecialPowerMask::NEUTRON_MISSILE, value),
            SpecialPowerType::SpySatellite => self.set(SpecialPowerMask::SPY_SATELLITE, value),
            SpecialPowerType::Defector => self.set(SpecialPowerMask::DEFECTOR, value),
            SpecialPowerType::TerrorCell => self.set(SpecialPowerMask::TERROR_CELL, value),
            SpecialPowerType::Ambush => self.set(SpecialPowerMask::AMBUSH, value),
            SpecialPowerType::BlackMarketNuke => {
                self.set(SpecialPowerMask::BLACK_MARKET_NUKE, value)
            }
            SpecialPowerType::AnthraxBomb => self.set(SpecialPowerMask::ANTHRAX_BOMB, value),
            SpecialPowerType::ScudStorm => self.set(SpecialPowerMask::SCUD_STORM, value),
            SpecialPowerType::Demoralize => self.set(SpecialPowerMask::DEMORALIZE, value),
            SpecialPowerType::CrateDrop => self.set(SpecialPowerMask::CRATE_DROP, value),
            SpecialPowerType::A10ThunderboltStrike => {
                self.set(SpecialPowerMask::A10_THUNDERBOLT_STRIKE, value)
            }
            SpecialPowerType::DetonateDirtyNuke => {
                self.set(SpecialPowerMask::DETONATE_DIRTY_NUKE, value)
            }
            SpecialPowerType::ArtilleryBarrage => {
                self.set(SpecialPowerMask::ARTILLERY_BARRAGE, value)
            }
            _ => {} // Other types not in mask
        }
    }
}
