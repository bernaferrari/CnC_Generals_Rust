//! Wave 92 residual peels: combat/sim residual deepen tables.
//!
//! Host-testable residual for fuller ZH combat simulation parity without full
//! INI archive load / exclusive module graph:
//! 1. Science residual full internal-name table (Science.ini)
//! 2. Body residual MaxHealth table for common units
//! 3. Combined pack honesty (weapon/armor/locomotor deepen live in their modules)
//!
//! Fail-closed:
//! - Not full ScienceStore NameKey purchase / prereq graph evaluation
//! - Not full ActiveBody ArmorSet swap / MaxHealthUpgrade exclusive modules
//! - Not network combat residual replication (network deferred)

// ---------------------------------------------------------------------------
// Science residual full name table (Science.ini — 96 retail entries)
// ---------------------------------------------------------------------------

/// Retail Science.ini residual count (Zero Hour).
pub const SCIENCE_RESIDUAL_NAME_COUNT: usize = 96;

/// Complete retail Science.ini internal-name residual table (declaration order).
///
/// Source: `Data/INI/Science.ini`. ScienceType keys are NameKey-generated at
/// load; this residual freezes the internal name set for host honesty.
pub const SCIENCE_RESIDUAL_NAME_TABLE: &[&str] = &[
    // Intrinsic faction
    "SCIENCE_AMERICA",
    "SCIENCE_CHINA",
    "SCIENCE_GLA",
    // Rank progression
    "SCIENCE_Rank1",
    "SCIENCE_Rank2",
    "SCIENCE_Rank3",
    "SCIENCE_Rank4",
    "SCIENCE_Rank5",
    "SCIENCE_Rank6",
    "SCIENCE_Rank7",
    "SCIENCE_Rank8",
    // America
    "SCIENCE_PaladinTank",
    "SCIENCE_StealthFighter",
    "SCIENCE_SpyDrone",
    "SCIENCE_Pathfinder",
    "SCIENCE_Paradrop1",
    "SCIENCE_Paradrop2",
    "SCIENCE_Paradrop3",
    "SCIENCE_A10ThunderboltMissileStrike1",
    "SCIENCE_A10ThunderboltMissileStrike2",
    "SCIENCE_A10ThunderboltMissileStrike3",
    "SCIENCE_SpectreGunshipSolo",
    "SCIENCE_SpectreGunship1",
    "SCIENCE_SpectreGunship2",
    "SCIENCE_SpectreGunship3",
    "SCIENCE_AirF_CarpetBomb",
    "SCIENCE_DaisyCutter",
    "SCIENCE_LeafletDrop",
    "Early_SCIENCE_LeafletDrop",
    "SCIENCE_MOAB",
    // China
    "SCIENCE_RedGuardTraining",
    "SCIENCE_BattlemasterTraining",
    "SCIENCE_ClusterMines",
    "SCIENCE_ArtilleryTraining",
    "SCIENCE_NukeLauncher",
    "SCIENCE_ArtilleryBarrage1",
    "SCIENCE_ArtilleryBarrage2",
    "SCIENCE_ArtilleryBarrage3",
    "SCIENCE_Frenzy1",
    "SCIENCE_Frenzy2",
    "SCIENCE_Frenzy3",
    "Early_SCIENCE_Frenzy1",
    "Early_SCIENCE_Frenzy2",
    "Early_SCIENCE_Frenzy3",
    "SCIENCE_CashHack1",
    "SCIENCE_CashHack2",
    "SCIENCE_CashHack3",
    "SCIENCE_EMPPulse",
    "SCIENCE_ChinaCarpetBomb",
    "Early_SCIENCE_ChinaCarpetBomb",
    "Nuke_SCIENCE_ChinaCarpetBomb",
    // GLA
    "SCIENCE_ScudLauncher",
    "SCIENCE_MarauderTank",
    "SCIENCE_TechnicalTraining",
    "SCIENCE_Hijacker",
    "SCIENCE_RebelAmbush1",
    "SCIENCE_RebelAmbush2",
    "SCIENCE_RebelAmbush3",
    "Chem_SCIENCE_RebelAmbush1",
    "Chem_SCIENCE_RebelAmbush2",
    "Chem_SCIENCE_RebelAmbush3",
    "SCIENCE_CashBounty1",
    "SCIENCE_CashBounty2",
    "SCIENCE_CashBounty3",
    "SCIENCE_AnthraxBomb",
    "SCIENCE_SneakAttack",
    "SCIENCE_GPSScrambler",
    "Slth_SCIENCE_GPSScrambler",
    // Shared
    "SCIENCE_EmergencyRepair1",
    "SCIENCE_EmergencyRepair2",
    "SCIENCE_EmergencyRepair3",
    "Early_SCIENCE_EmergencyRepair1",
    "Early_SCIENCE_EmergencyRepair2",
    "Early_SCIENCE_EmergencyRepair3",
    // Unused / reserved residual
    "SCIENCE_BlackMarketNuke",
    "SCIENCE_CrateDrop",
    "SCIENCE_CarpetBomb",
    "SCIENCE_NapalmStrike",
    "SCIENCE_Defector",
    "SCIENCE_TerrorCell",
    // Generals expand residual
    "SCIENCE_OverlordTraining",
    "SCIENCE_GattlingTankTraining",
    "SCIENCE_TankParadrop1",
    "SCIENCE_TankParadrop2",
    "SCIENCE_TankParadrop3",
    "Infa_SCIENCE_RedGuardTraining",
    "SCIENCE_InfantryParadrop1",
    "SCIENCE_InfantryParadrop2",
    "SCIENCE_InfantryParadrop3",
    "Infa_SCIENCE_InfantryParadrop1",
    "Infa_SCIENCE_InfantryParadrop2",
    "Infa_SCIENCE_InfantryParadrop3",
    "Nuke_SCIENCE_NukeDrop",
    "AirF_SCIENCE_A10ThunderboltMissileStrike1",
    "AirF_SCIENCE_A10ThunderboltMissileStrike2",
    "AirF_SCIENCE_A10ThunderboltMissileStrike3",
];

/// Whether a science internal name is in the retail residual table.
pub fn science_residual_name_known(name: &str) -> bool {
    SCIENCE_RESIDUAL_NAME_TABLE.iter().any(|n| *n == name)
}

/// Wave 92 honesty: Science.ini full residual name table completeness.
///
/// Fail-closed: not full ScienceStore purchase cost / prereq evaluation.
pub fn honesty_science_name_table_residual_wave92() -> bool {
    if SCIENCE_RESIDUAL_NAME_TABLE.len() != SCIENCE_RESIDUAL_NAME_COUNT {
        return false;
    }
    // Unique names.
    let mut seen = std::collections::HashSet::new();
    for n in SCIENCE_RESIDUAL_NAME_TABLE {
        if !seen.insert(*n) {
            return false;
        }
        if !n.contains("SCIENCE") && !n.starts_with("Early_") && !n.starts_with("Chem_")
            && !n.starts_with("Slth_") && !n.starts_with("Nuke_") && !n.starts_with("AirF_")
            && !n.starts_with("Infa_")
        {
            // All residual names contain SCIENCE token or known general prefix.
            if !n.contains("SCIENCE") {
                return false;
            }
        }
    }
    // Faction + rank + key purchasable anchors.
    let anchors = [
        "SCIENCE_AMERICA",
        "SCIENCE_CHINA",
        "SCIENCE_GLA",
        "SCIENCE_Rank1",
        "SCIENCE_Rank5",
        "SCIENCE_Rank8",
        "SCIENCE_PaladinTank",
        "SCIENCE_StealthFighter",
        "SCIENCE_Pathfinder",
        "SCIENCE_DaisyCutter",
        "SCIENCE_MOAB",
        "SCIENCE_NukeLauncher",
        "SCIENCE_ClusterMines",
        "SCIENCE_EMPPulse",
        "SCIENCE_ScudLauncher",
        "SCIENCE_MarauderTank",
        "SCIENCE_CashBounty3",
        "SCIENCE_AnthraxBomb",
        "SCIENCE_GPSScrambler",
        "SCIENCE_EmergencyRepair3",
        "SCIENCE_OverlordTraining",
        "SCIENCE_GattlingTankTraining",
        "AirF_SCIENCE_A10ThunderboltMissileStrike3",
        "Infa_SCIENCE_InfantryParadrop3",
        "Nuke_SCIENCE_NukeDrop",
    ];
    anchors.iter().all(|a| science_residual_name_known(a))
        && SCIENCE_RESIDUAL_NAME_TABLE[0] == "SCIENCE_AMERICA"
        && SCIENCE_RESIDUAL_NAME_TABLE[2] == "SCIENCE_GLA"
        && SCIENCE_RESIDUAL_NAME_TABLE[3] == "SCIENCE_Rank1"
        && SCIENCE_RESIDUAL_NAME_COUNT == 96
}

// ---------------------------------------------------------------------------
// Body residual MaxHealth table for common units
// ---------------------------------------------------------------------------

/// Common-unit body residual row: (template anchor name, MaxHealth).
///
/// Values match retail Object.ini ActiveBody MaxHealth residual (or existing
/// host residual constants). Fail-closed: not full body module graph.
pub const HOST_BODY_MAX_HEALTH_RESIDUAL_TABLE_WAVE92: &[(&str, f32)] = &[
    // Infantry
    ("AmericaInfantryRanger", 180.0),
    ("GLAInfantryRebel", 120.0),
    ("ChinaInfantryRedguard", 120.0),
    ("AmericaInfantryMissileDefender", 100.0),
    ("AmericaInfantryPathfinder", 120.0),
    ("ChinaInfantryTankHunter", 100.0),
    ("AmericaInfantryColonelBurton", 200.0),
    ("GLAInfantryJarmenKell", 200.0),
    ("ChinaInfantryBlackLotus", 200.0),
    ("GLAInfantryWorker", 100.0),
    ("AmericaInfantryPilot", 100.0),
    // Vehicles USA
    ("AmericaVehicleHumvee", 240.0),
    ("AmericaTankCrusader", 480.0),
    ("AmericaTankPaladin", 500.0),
    ("AmericaVehicleTomahawk", 180.0),
    ("AmericaTankAvenger", 300.0),
    ("AmericaVehicleDozer", 250.0),
    // Vehicles GLA
    ("GLAVehicleTechnical", 180.0),
    ("GLATankScorpion", 370.0),
    ("GLATankMarauder", 500.0),
    ("GLAVehicleQuadCannon", 300.0),
    ("GLAVehicleScudLauncher", 180.0),
    ("GLAVehicleRocketBuggy", 120.0),
    ("GLAVehicleBattleBus", 400.0),
    ("GLAVehicleBombTruck", 220.0),
    ("GLAVehicleRadarVan", 200.0),
    // Vehicles China
    ("ChinaTankBattleMaster", 400.0),
    ("ChinaTankOverlord", 1100.0),
    ("ChinaTankGattling", 300.0),
    ("ChinaTankDragon", 280.0),
    ("ChinaVehicleNukeLauncher", 240.0),
    ("ChinaVehicleInfernoCannon", 120.0),
    // Aircraft
    ("AmericaJetRaptor", 160.0),
    ("AmericaJetStealthFighter", 120.0),
    ("AmericaJetAurora", 80.0),
    ("AmericaVehicleComanche", 220.0),
    ("ChinaJetMIG", 160.0),
    ("ChinaVehicleHelix", 300.0),
    // Structures (economy anchors)
    ("AmericaCommandCenter", 5000.0),
    ("AmericaPowerPlant", 800.0),
    ("AmericaDozer", 250.0),
    ("SupplyWarehouse", 1000.0),
    ("AmericaPatriotBattery", 1000.0),
    ("GLAStingerSite", 1000.0),
];

/// Lookup residual MaxHealth for a common unit template name.
pub fn body_max_health_residual(template_name: &str) -> Option<f32> {
    HOST_BODY_MAX_HEALTH_RESIDUAL_TABLE_WAVE92
        .iter()
        .find(|(n, _)| *n == template_name)
        .map(|(_, h)| *h)
}

/// Wave 92 honesty: common-unit body MaxHealth residual table.
///
/// Cross-checks key host residual constants where they already exist.
/// Fail-closed: not full ActiveBody / ArmorSet / MaxHealthUpgrade matrix.
pub fn honesty_body_max_health_residual_table_wave92() -> bool {
    let table = HOST_BODY_MAX_HEALTH_RESIDUAL_TABLE_WAVE92;
    if table.len() < 40 {
        return false;
    }
    // Unique names.
    let mut seen = std::collections::HashSet::new();
    for (n, h) in table {
        if !seen.insert(*n) || *h <= 0.0 {
            return false;
        }
    }
    let eq = |name: &str, expected: f32| {
        body_max_health_residual(name)
            .map(|h| (h - expected).abs() < 0.01)
            .unwrap_or(false)
    };
    // Cross-check existing host residual constants.
    use crate::game_logic::host_battlemaster::BATTLE_MASTER_MAX_HEALTH;
    use crate::game_logic::host_gla_rebel::REBEL_MAX_HEALTH;
    use crate::game_logic::host_humvee::HUMVEE_MAX_HEALTH;
    use crate::game_logic::host_marauder::MARAUDER_MAX_HEALTH;
    use crate::game_logic::host_overlord_gun::OVERLORD_MAX_HEALTH;
    use crate::game_logic::host_ranger::RANGER_MAX_HEALTH;
    use crate::game_logic::host_raptor::RAPTOR_MAX_HEALTH;
    use crate::game_logic::host_red_guard::REDGUARD_MAX_HEALTH;
    use crate::game_logic::host_rocket_buggy::BUGGY_MAX_HEALTH;
    use crate::game_logic::host_structure_economy_residual::{
        COMMAND_CENTER_MAX_HEALTH, DOZER_MAX_HEALTH,
    };
    use crate::game_logic::host_technical::TECHNICAL_MAX_HEALTH;
    use crate::game_logic::host_tomahawk::TOMAHAWK_MAX_HEALTH;
    use crate::game_logic::host_usa_tanks::{CRUSADER_MAX_HEALTH, PALADIN_MAX_HEALTH};

    eq("AmericaInfantryRanger", 180.0)
        && eq("GLAInfantryRebel", 120.0)
        && eq("ChinaInfantryRedguard", 120.0)
        && eq("AmericaVehicleHumvee", 240.0)
        && eq("AmericaTankCrusader", 480.0)
        && eq("AmericaTankPaladin", 500.0)
        && eq("AmericaVehicleTomahawk", 180.0)
        && eq("GLAVehicleTechnical", 180.0)
        && eq("GLATankScorpion", 370.0)
        && eq("GLATankMarauder", 500.0)
        && eq("GLAVehicleRocketBuggy", 120.0)
        && eq("ChinaTankBattleMaster", 400.0)
        && eq("ChinaTankOverlord", 1100.0)
        && eq("ChinaTankDragon", 280.0)
        && eq("AmericaJetRaptor", 160.0)
        && eq("AmericaVehicleComanche", 220.0)
        && eq("ChinaJetMIG", 160.0)
        && eq("AmericaCommandCenter", 5000.0)
        && eq("AmericaDozer", 250.0)
        // Host constant parity residual
        && (RANGER_MAX_HEALTH - 180.0).abs() < 0.01
        && (REBEL_MAX_HEALTH - 120.0).abs() < 0.01
        && (REDGUARD_MAX_HEALTH - 120.0).abs() < 0.01
        && (HUMVEE_MAX_HEALTH - 240.0).abs() < 0.01
        && (CRUSADER_MAX_HEALTH - 480.0).abs() < 0.01
        && (PALADIN_MAX_HEALTH - 500.0).abs() < 0.01
        && (TOMAHAWK_MAX_HEALTH - 180.0).abs() < 0.01
        && (TECHNICAL_MAX_HEALTH - 180.0).abs() < 0.01
        && (MARAUDER_MAX_HEALTH - 500.0).abs() < 0.01
        && (BUGGY_MAX_HEALTH - 120.0).abs() < 0.01
        && (BATTLE_MASTER_MAX_HEALTH - 400.0).abs() < 0.01
        && (OVERLORD_MAX_HEALTH - 1100.0).abs() < 0.01
        && (RAPTOR_MAX_HEALTH - 160.0).abs() < 0.01
        && (COMMAND_CENTER_MAX_HEALTH - 5000.0).abs() < 0.01
        && (DOZER_MAX_HEALTH - 250.0).abs() < 0.01
}

/// Combined Wave 92 combat/sim residual pack honesty.
///
/// Aggregates science name table + body MaxHealth table + weapon/armor/locomotor
/// deepen residual honesty from their home modules.
pub fn honesty_combat_sim_residual_pack_wave92() -> bool {
    honesty_science_name_table_residual_wave92()
        && honesty_body_max_health_residual_table_wave92()
        && crate::game_logic::weapon_bootstrap::honesty_weapon_store_deepen_residual_wave92()
        && crate::game_logic::host_armor_residual::honesty_armor_residual_expand_wave92()
        && crate::game_logic::locomotor_bootstrap::honesty_locomotor_residual_expand_wave92()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn science_name_table_wave92_honesty() {
        assert!(honesty_science_name_table_residual_wave92());
        assert_eq!(SCIENCE_RESIDUAL_NAME_TABLE.len(), 96);
        assert!(science_residual_name_known("SCIENCE_PaladinTank"));
        assert!(science_residual_name_known("SCIENCE_CashBounty3"));
        assert!(!science_residual_name_known("SCIENCE_DoesNotExist"));
    }

    #[test]
    fn body_max_health_table_wave92_honesty() {
        assert!(honesty_body_max_health_residual_table_wave92());
        assert_eq!(body_max_health_residual("AmericaTankCrusader"), Some(480.0));
        assert_eq!(body_max_health_residual("GLATankScorpion"), Some(370.0));
        assert_eq!(body_max_health_residual("ChinaTankDragon"), Some(280.0));
        assert_eq!(body_max_health_residual("AmericaVehicleComanche"), Some(220.0));
        assert!(body_max_health_residual("NotAUnit").is_none());
    }

    #[test]
    fn combat_sim_residual_pack_wave92_honesty() {
        assert!(honesty_combat_sim_residual_pack_wave92());
    }
}
