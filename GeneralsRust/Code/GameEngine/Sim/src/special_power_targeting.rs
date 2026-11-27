//! C++ ActionManager target rules after the source and readiness checks.

use crate::special_power_type::SpecialPowerType;

enum LocationRule {
    UnshroudedLand,
    Unshrouded,
    OnMap,
    Anywhere,
    Unavailable,
}

fn location_rule(power: SpecialPowerType) -> LocationRule {
    match power {
        SpecialPowerType::ParadropAmerica
        | SpecialPowerType::InfaParadropAmerica
        | SpecialPowerType::CrateDrop
        | SpecialPowerType::TankParadrop => LocationRule::UnshroudedLand,
        SpecialPowerType::DaisyCutter
        | SpecialPowerType::AirfDaisyCutter
        | SpecialPowerType::CarpetBomb
        | SpecialPowerType::ChinaCarpetBomb
        | SpecialPowerType::LeafletDrop
        | SpecialPowerType::EarlyLeafletDrop
        | SpecialPowerType::EarlyChinaCarpetBomb
        | SpecialPowerType::AirfCarpetBomb
        | SpecialPowerType::SuprCruiseMissile
        | SpecialPowerType::ClusterMines
        | SpecialPowerType::NukeClusterMines
        | SpecialPowerType::EmpPulse
        | SpecialPowerType::NapalmStrike
        | SpecialPowerType::BlackMarketNuke
        | SpecialPowerType::AnthraxBomb
        | SpecialPowerType::TerrorCell
        | SpecialPowerType::Ambush
        | SpecialPowerType::NeutronMissile
        | SpecialPowerType::NukeNeutronMissile
        | SpecialPowerType::SupwNeutronMissile
        | SpecialPowerType::ScudStorm
        | SpecialPowerType::Demoralize
        | SpecialPowerType::A10ThunderboltStrike
        | SpecialPowerType::AirfA10ThunderboltStrike
        | SpecialPowerType::SpectreGunship
        | SpecialPowerType::AirfSpectreGunship
        | SpecialPowerType::RepairVehicles
        | SpecialPowerType::EarlyRepairVehicles
        | SpecialPowerType::GpsScrambler
        | SpecialPowerType::SlthGpsScrambler
        | SpecialPowerType::ArtilleryBarrage
        | SpecialPowerType::Frenzy
        | SpecialPowerType::EarlyFrenzy
        | SpecialPowerType::ParticleUplinkCannon
        | SpecialPowerType::SupwParticleUplinkCannon
        | SpecialPowerType::LazrParticleUplinkCannon
        | SpecialPowerType::CleanupArea
        | SpecialPowerType::SneakAttack
        | SpecialPowerType::BattleshipBombardment => LocationRule::Unshrouded,
        SpecialPowerType::SpySatellite
        | SpecialPowerType::RadarVanScan
        | SpecialPowerType::SpyDrone
        | SpecialPowerType::HelixNapalmBomb => LocationRule::OnMap,
        SpecialPowerType::LaunchBaikonurRocket => LocationRule::Anywhere,
        _ => LocationRule::Unavailable,
    }
}

/// C++ ActionManager::canDoSpecialPowerAtLocation evaluates only the queries
/// needed for the selected power. Drops reject water before checking shroud.
pub fn allows_location(
    power: SpecialPowerType,
    underwater: impl FnOnce() -> bool,
    shrouded: impl FnOnce() -> bool,
    on_map: impl FnOnce() -> bool,
) -> bool {
    match location_rule(power) {
        LocationRule::UnshroudedLand => !underwater() && !shrouded(),
        LocationRule::Unshrouded => !shrouded(),
        LocationRule::OnMap => on_map(),
        LocationRule::Anywhere => true,
        LocationRule::Unavailable => false,
    }
}

/// CommandXlat keeps the dual-path battleship, object-charge Helix, and
/// no-target Baikonur rocket out of the location-only set.
pub fn is_location_target_only(power: SpecialPowerType) -> bool {
    !matches!(
        power,
        SpecialPowerType::BattleshipBombardment | SpecialPowerType::HelixNapalmBomb
    ) && matches!(
        location_rule(power),
        LocationRule::UnshroudedLand | LocationRule::Unshrouded | LocationRule::OnMap
    )
}

/// C++ ActionManager::canDoSpecialPower accepts these commands without a target.
pub fn allows_no_target(power: SpecialPowerType) -> bool {
    matches!(
        power,
        SpecialPowerType::RemoteCharges
            | SpecialPowerType::CiaIntelligence
            | SpecialPowerType::CommunicationsDownload
            | SpecialPowerType::DetonateDirtyNuke
            | SpecialPowerType::ChangeBattlePlans
            | SpecialPowerType::LaunchBaikonurRocket
    )
}
