//! Wave 103 residual peels: weapon / armor / locomotor / special-power /
//! object KindOf residual deepen (host-testable game-logic residual).
//!
//! Orthogonal to Waves 77/81/92 (core weapon/armor/loco seeds) and Wave 80
//! (superweapon building KindOf + SpecialPower enum). Host residual only —
//! shell `playable_claim` stays false; network deferred.
//!
//! Sources (retail ZH INI):
//! - Weapon.ini residual deepen beyond Wave 92 (NukeCannon / Inferno / Aurora /
//!   FireBase / Sentry / Hellfire / JarmenKell / TunnelDefender / MiniGunner /
//!   Overlord / BattleMaster / Comanche AT+pods / Avenger AA / SCUD toxin /
//!   BlackNapalm)
//! - Armor.ini specialized templates beyond Wave 92 (HazMat / ChemSuit / Dozer /
//!   UpgradedTank / Humvee / Dragon / ToxinTruck / Comanche / StructureTough)
//! - Locomotor.ini names beyond Wave 92 (BombTruck / TroopCrawler / RadarVan /
//!   ToxinTruck / Chinook / A10 / B52 / CombatBike / POWTruck /
//!   NuclearBattleMaster / JarmenKell / BlackLotus / Saboteur / MissileDefender)
//! - SpecialPower.ini Superweapon residual name / Enum / ReloadTime table for
//!   powers not fully closed on HostSuperweaponKind (MOAB / EMP / Napalm /
//!   BlackMarketNuke / TerrorCell / CrateDrop / Frenzy / CashHack / DirtyNuke /
//!   Leaflet / SpySatellite / SpyDrone / RadarVan / EmergencyRepair / GPS / CIA /
//!   SneakAttack / Ambush / Baikonur / SupW PUC)
//! - Object/*.ini KindOf residual packs for common unit / structure types
//!
//! Fail-closed:
//! - Not full Weapon.ini / Armor.ini / Locomotor.ini archive parse residual
//! - Not full SpecialPowerStore SharedSyncedTimer / PublicTimer UI residual
//! - Not full ThingTemplate KindOf bit matrix / live INI parse residual
//! - Shell `playable_claim` stays false; network deferred

use crate::game_logic::host_armor_residual::honesty_armor_residual_expand_wave103;
use crate::game_logic::host_special_power_enum_residual::{
    special_power_bit_name_index, SPECIAL_POWER_BIT_NAME_LIST,
};
use crate::game_logic::locomotor_bootstrap::honesty_locomotor_residual_expand_wave103;
use crate::game_logic::special_power_strikes::duration_ms_to_logic_frames;
use crate::game_logic::weapon_bootstrap::honesty_weapon_store_deepen_residual_wave103;

// ---------------------------------------------------------------------------
// 1. SpecialPower.ini superweapon residual deepen (beyond HostSuperweaponKind)
// ---------------------------------------------------------------------------

/// Wave 103 SpecialPower.ini residual table: (template name, Enum, ReloadTime ms).
///
/// Incomplete / non-HostSuperweaponKind superweapon residual deepen for
/// host-testable SpecialPower.ini name + Enum + ReloadTime residual.
pub const SUPERWEAPON_SPECIAL_POWER_RELOAD_TABLE_WAVE103: &[(&str, &str, u32)] = &[
    ("SuperweaponMOAB", "SPECIAL_DAISY_CUTTER", 360_000),
    ("SuperweaponEMPPulse", "SPECIAL_EMP_PULSE", 360_000),
    ("SuperweaponNapalmStrike", "SPECIAL_NAPALM_STRIKE", 600_000),
    ("SuperweaponBlackMarketNuke", "SPECIAL_BLACK_MARKET_NUKE", 600_000),
    ("SuperweaponTerrorCell", "SPECIAL_TERROR_CELL", 600_000),
    ("SuperweaponCrateDrop", "SPECIAL_CRATE_DROP", 600_000),
    ("SuperweaponFrenzy", "SPECIAL_FRENZY", 240_000),
    ("SuperweaponCashHack", "SPECIAL_CASH_HACK", 240_000),
    ("SuperweaponDetonateDirtyNuke", "SPECIAL_DETONATE_DIRTY_NUKE", 30_000),
    ("SuperweaponLeafletDrop", "SPECIAL_LEAFLET_DROP", 300_000),
    ("SpecialPowerSpySatellite", "SPECIAL_SPY_SATELLITE", 60_000),
    ("SpecialPowerSpyDrone", "SPECIAL_SPY_DRONE", 90_000),
    ("SpecialPowerRadarVanScan", "SPECIAL_RADAR_VAN_SCAN", 30_000),
    ("SuperweaponEmergencyRepair", "SPECIAL_REPAIR_VEHICLES", 240_000),
    ("SuperweaponGPSScrambler", "SPECIAL_GPS_SCRAMBLER", 240_000),
    ("SuperweaponCIAIntelligence", "SPECIAL_CIA_INTELLIGENCE", 300_000),
    ("SuperweaponSneakAttack", "SPECIAL_SNEAK_ATTACK", 150_000),
    ("SuperweaponRebelAmbush", "SPECIAL_AMBUSH", 240_000),
    ("SuperweaponLaunchBaikonurRocket", "SPECIAL_LAUNCH_BAIKONUR_ROCKET", 0),
    (
        "SupW_SuperweaponParticleUplinkCannon",
        "SUPW_SPECIAL_PARTICLE_UPLINK_CANNON",
        180_000,
    ),
];

/// Lookup Superweapon SpecialPower.ini residual row by template name.
pub fn superweapon_special_power_row_wave103(
    template: &str,
) -> Option<(&'static str, &'static str, u32)> {
    SUPERWEAPON_SPECIAL_POWER_RELOAD_TABLE_WAVE103
        .iter()
        .copied()
        .find(|(name, _, _)| *name == template)
}

/// Wave 103 honesty: Superweapon SpecialPower.ini residual deepen pack.
///
/// Freezes template name / Enum / ReloadTime residual for powers incomplete on
/// HostSuperweaponKind (MOAB / EMP / Napalm / support powers / faction variants).
/// Cross-links Enum strings into Wave 80 SPECIAL_POWER_BIT_NAME_LIST.
/// Fail-closed: not full SpecialPowerStore load / SharedSyncedTimer residual.
pub fn honesty_special_power_superweapon_residual_deepen_wave103() -> bool {
    if SUPERWEAPON_SPECIAL_POWER_RELOAD_TABLE_WAVE103.len() < 20 {
        return false;
    }
    // Unique template names.
    let mut names: Vec<&str> = SUPERWEAPON_SPECIAL_POWER_RELOAD_TABLE_WAVE103
        .iter()
        .map(|(n, _, _)| *n)
        .collect();
    names.sort_unstable();
    if names.windows(2).any(|w| w[0] == w[1]) {
        return false;
    }
    // Key residual anchors (SpecialPower.ini).
    let moab = superweapon_special_power_row_wave103("SuperweaponMOAB");
    let emp = superweapon_special_power_row_wave103("SuperweaponEMPPulse");
    let napalm = superweapon_special_power_row_wave103("SuperweaponNapalmStrike");
    let dirty = superweapon_special_power_row_wave103("SuperweaponDetonateDirtyNuke");
    let spy = superweapon_special_power_row_wave103("SpecialPowerSpySatellite");
    let baikonur = superweapon_special_power_row_wave103("SuperweaponLaunchBaikonurRocket");
    let supw_puc =
        superweapon_special_power_row_wave103("SupW_SuperweaponParticleUplinkCannon");
    let anchors_ok = matches!(moab, Some(("SuperweaponMOAB", "SPECIAL_DAISY_CUTTER", 360_000)))
        && matches!(emp, Some(("SuperweaponEMPPulse", "SPECIAL_EMP_PULSE", 360_000)))
        && matches!(
            napalm,
            Some(("SuperweaponNapalmStrike", "SPECIAL_NAPALM_STRIKE", 600_000))
        )
        && matches!(
            dirty,
            Some((
                "SuperweaponDetonateDirtyNuke",
                "SPECIAL_DETONATE_DIRTY_NUKE",
                30_000
            ))
        )
        && matches!(
            spy,
            Some(("SpecialPowerSpySatellite", "SPECIAL_SPY_SATELLITE", 60_000))
        )
        && matches!(
            baikonur,
            Some((
                "SuperweaponLaunchBaikonurRocket",
                "SPECIAL_LAUNCH_BAIKONUR_ROCKET",
                0
            ))
        )
        && matches!(
            supw_puc,
            Some((
                "SupW_SuperweaponParticleUplinkCannon",
                "SUPW_SPECIAL_PARTICLE_UPLINK_CANNON",
                180_000
            ))
        );
    if !anchors_ok {
        return false;
    }
    // Enum residual present in Wave 80 bit-name list; ReloadTime → frames residual.
    let enum_reload_ok = SUPERWEAPON_SPECIAL_POWER_RELOAD_TABLE_WAVE103
        .iter()
        .all(|(_, enum_name, reload_ms)| {
            let in_list = special_power_bit_name_index(enum_name).is_some()
                || SPECIAL_POWER_BIT_NAME_LIST.contains(enum_name);
            let frames = duration_ms_to_logic_frames(*reload_ms);
            // 0 ms → 0 frames; else ceil(ms*30/1000) residual.
            let expected = if *reload_ms == 0 {
                0
            } else {
                ((*reload_ms as u64 * 30 + 999) / 1000) as u32
            };
            in_list && frames == expected
        });
    // Ordering residual: DirtyNuke 30s < SpySat 60s < Sneak 150s < SupW PUC 180s
    // < Frenzy/CashHack/GPS/Repair 240s < Leaflet/CIA 300s < MOAB/EMP 360s
    // < Napalm/Terror/Crate/BlackMarket 600s.
    let dirty_ms = 30_000u32;
    let spy_ms = 60_000u32;
    let sneak_ms = 150_000u32;
    let supw_ms = 180_000u32;
    let frenzy_ms = 240_000u32;
    let leaflet_ms = 300_000u32;
    let moab_ms = 360_000u32;
    let napalm_ms = 600_000u32;
    anchors_ok
        && enum_reload_ok
        && dirty_ms < spy_ms
        && spy_ms < sneak_ms
        && sneak_ms < supw_ms
        && supw_ms < frenzy_ms
        && frenzy_ms < leaflet_ms
        && leaflet_ms < moab_ms
        && moab_ms < napalm_ms
        && duration_ms_to_logic_frames(dirty_ms) == 900
        && duration_ms_to_logic_frames(spy_ms) == 1_800
        && duration_ms_to_logic_frames(moab_ms) == 10_800
        && duration_ms_to_logic_frames(napalm_ms) == 18_000
}

// ---------------------------------------------------------------------------
// 2. Object residual KindOf packs for more unit types
// ---------------------------------------------------------------------------

/// Common unit / structure Object KindOf residual pack (FactionUnit / Object INI).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectKindOfResidualPack {
    pub object_name: &'static str,
    pub kind_of: &'static str,
    pub build_cost: i32,
    pub build_time_sec: f32,
    pub max_health: f32,
    pub is_infantry: bool,
    pub is_vehicle: bool,
    pub is_aircraft: bool,
    pub is_structure: bool,
    pub is_transport: bool,
    pub is_salvager: bool,
}

/// Wave 103 Object KindOf residual packs for common unit / structure types.
pub const OBJECT_KINDOF_RESIDUAL_PACKS_WAVE103: &[ObjectKindOfResidualPack] = &[
    ObjectKindOfResidualPack {
        object_name: "AmericaInfantryRanger",
        kind_of: "PRELOAD SELECTABLE CAN_ATTACK ATTACK_NEEDS_LINE_OF_SIGHT CAN_CAST_REFLECTIONS INFANTRY CAN_RAPPEL SCORE",
        build_cost: 225,
        build_time_sec: 5.0,
        max_health: 180.0,
        is_infantry: true,
        is_vehicle: false,
        is_aircraft: false,
        is_structure: false,
        is_transport: false,
        is_salvager: false,
    },
    ObjectKindOfResidualPack {
        object_name: "GLAInfantryRebel",
        kind_of: "PRELOAD SELECTABLE CAN_ATTACK ATTACK_NEEDS_LINE_OF_SIGHT CAN_CAST_REFLECTIONS INFANTRY SALVAGER SCORE",
        build_cost: 150,
        build_time_sec: 5.0,
        max_health: 120.0,
        is_infantry: true,
        is_vehicle: false,
        is_aircraft: false,
        is_structure: false,
        is_transport: false,
        is_salvager: true,
    },
    ObjectKindOfResidualPack {
        object_name: "ChinaInfantryRedguard",
        kind_of: "PRELOAD SELECTABLE CAN_ATTACK ATTACK_NEEDS_LINE_OF_SIGHT CAN_CAST_REFLECTIONS INFANTRY SCORE PARACHUTABLE",
        build_cost: 300,
        build_time_sec: 10.0,
        max_health: 120.0,
        is_infantry: true,
        is_vehicle: false,
        is_aircraft: false,
        is_structure: false,
        is_transport: false,
        is_salvager: false,
    },
    ObjectKindOfResidualPack {
        object_name: "AmericaTankCrusader",
        kind_of: "PRELOAD SELECTABLE CAN_ATTACK ATTACK_NEEDS_LINE_OF_SIGHT CAN_CAST_REFLECTIONS VEHICLE SCORE",
        build_cost: 900,
        build_time_sec: 10.0,
        max_health: 480.0,
        is_infantry: false,
        is_vehicle: true,
        is_aircraft: false,
        is_structure: false,
        is_transport: false,
        is_salvager: false,
    },
    ObjectKindOfResidualPack {
        object_name: "ChinaTankBattleMaster",
        kind_of: "PRELOAD SELECTABLE CAN_ATTACK ATTACK_NEEDS_LINE_OF_SIGHT CAN_CAST_REFLECTIONS VEHICLE SCORE",
        build_cost: 800,
        build_time_sec: 10.0,
        max_health: 400.0,
        is_infantry: false,
        is_vehicle: true,
        is_aircraft: false,
        is_structure: false,
        is_transport: false,
        is_salvager: false,
    },
    ObjectKindOfResidualPack {
        object_name: "GLATankScorpion",
        kind_of: "PRELOAD SELECTABLE CAN_ATTACK ATTACK_NEEDS_LINE_OF_SIGHT CAN_CAST_REFLECTIONS VEHICLE SALVAGER WEAPON_SALVAGER SCORE",
        build_cost: 600,
        build_time_sec: 7.0,
        max_health: 370.0,
        is_infantry: false,
        is_vehicle: true,
        is_aircraft: false,
        is_structure: false,
        is_transport: false,
        is_salvager: true,
    },
    ObjectKindOfResidualPack {
        object_name: "AmericaVehicleHumvee",
        kind_of: "PRELOAD SELECTABLE CAN_ATTACK ATTACK_NEEDS_LINE_OF_SIGHT CAN_CAST_REFLECTIONS VEHICLE SCORE TRANSPORT",
        build_cost: 700,
        build_time_sec: 10.0,
        max_health: 240.0,
        is_infantry: false,
        is_vehicle: true,
        is_aircraft: false,
        is_structure: false,
        is_transport: true,
        is_salvager: false,
    },
    ObjectKindOfResidualPack {
        object_name: "GLAVehicleTechnical",
        kind_of: "PRELOAD SELECTABLE CAN_ATTACK ATTACK_NEEDS_LINE_OF_SIGHT CAN_CAST_REFLECTIONS SALVAGER WEAPON_SALVAGER VEHICLE TRANSPORT",
        build_cost: 500,
        build_time_sec: 5.0,
        max_health: 180.0,
        is_infantry: false,
        is_vehicle: true,
        is_aircraft: false,
        is_structure: false,
        is_transport: true,
        is_salvager: true,
    },
    ObjectKindOfResidualPack {
        object_name: "AmericaJetRaptor",
        kind_of: "PRELOAD CAN_CAST_REFLECTIONS CAN_ATTACK SELECTABLE VEHICLE SCORE AIRCRAFT",
        build_cost: 1400,
        build_time_sec: 20.0,
        max_health: 160.0,
        is_infantry: false,
        is_vehicle: true,
        is_aircraft: true,
        is_structure: false,
        is_transport: false,
        is_salvager: false,
    },
    ObjectKindOfResidualPack {
        object_name: "AmericaVehicleComanche",
        kind_of: "PRELOAD CAN_CAST_REFLECTIONS CAN_ATTACK SELECTABLE VEHICLE SCORE AIRCRAFT PRODUCED_AT_HELIPAD",
        build_cost: 1500,
        build_time_sec: 20.0,
        max_health: 220.0,
        is_infantry: false,
        is_vehicle: true,
        is_aircraft: true,
        is_structure: false,
        is_transport: false,
        is_salvager: false,
    },
    ObjectKindOfResidualPack {
        object_name: "ChinaVehicleHelix",
        kind_of: "PRELOAD CAN_CAST_REFLECTIONS SELECTABLE VEHICLE HUGE_VEHICLE TRANSPORT AIRCRAFT SCORE PRODUCED_AT_HELIPAD CAN_ATTACK",
        build_cost: 1500,
        build_time_sec: 20.0,
        max_health: 300.0,
        is_infantry: false,
        is_vehicle: true,
        is_aircraft: true,
        is_structure: false,
        is_transport: true,
        is_salvager: false,
    },
    ObjectKindOfResidualPack {
        object_name: "ChinaTankOverlord",
        kind_of: "PRELOAD SELECTABLE CAN_ATTACK ATTACK_NEEDS_LINE_OF_SIGHT CAN_CAST_REFLECTIONS VEHICLE SCORE HUGE_VEHICLE",
        build_cost: 2000,
        build_time_sec: 20.0,
        max_health: 1100.0,
        is_infantry: false,
        is_vehicle: true,
        is_aircraft: false,
        is_structure: false,
        is_transport: false,
        is_salvager: false,
    },
    ObjectKindOfResidualPack {
        object_name: "AmericaVehicleTomahawk",
        kind_of: "PRELOAD SELECTABLE DONT_AUTO_CRUSH_INFANTRY CAN_ATTACK CAN_CAST_REFLECTIONS VEHICLE SCORE",
        build_cost: 1200,
        build_time_sec: 20.0,
        max_health: 180.0,
        is_infantry: false,
        is_vehicle: true,
        is_aircraft: false,
        is_structure: false,
        is_transport: false,
        is_salvager: false,
    },
    ObjectKindOfResidualPack {
        object_name: "ChinaInfantryHacker",
        kind_of: "PRELOAD SELECTABLE CAN_ATTACK CAN_CAST_REFLECTIONS INFANTRY SCORE IGNORES_SELECT_ALL MONEY_HACKER",
        build_cost: 625,
        build_time_sec: 20.0,
        max_health: 100.0,
        is_infantry: true,
        is_vehicle: false,
        is_aircraft: false,
        is_structure: false,
        is_transport: false,
        is_salvager: false,
    },
    ObjectKindOfResidualPack {
        object_name: "AmericaCommandCenter",
        kind_of: "PRELOAD STRUCTURE SELECTABLE IMMOBILE COMMANDCENTER SCORE CAPTURABLE FS_FACTORY AUTO_RALLYPOINT MP_COUNT_FOR_VICTORY",
        build_cost: 2000,
        build_time_sec: 45.0,
        max_health: 5000.0,
        is_infantry: false,
        is_vehicle: false,
        is_aircraft: false,
        is_structure: true,
        is_transport: false,
        is_salvager: false,
    },
    ObjectKindOfResidualPack {
        object_name: "ChinaWarFactory",
        kind_of: "PRELOAD STRUCTURE SELECTABLE IMMOBILE REPAIR_PAD SCORE CAPTURABLE FS_FACTORY AUTO_RALLYPOINT MP_COUNT_FOR_VICTORY FS_WARFACTORY",
        build_cost: 2000,
        build_time_sec: 15.0,
        max_health: 2000.0,
        is_infantry: false,
        is_vehicle: false,
        is_aircraft: false,
        is_structure: true,
        is_transport: false,
        is_salvager: false,
    },
    ObjectKindOfResidualPack {
        object_name: "GLABarracks",
        kind_of: "PRELOAD STRUCTURE SELECTABLE IMMOBILE HEAL_PAD CAPTURABLE FS_FACTORY AUTO_RALLYPOINT MP_COUNT_FOR_VICTORY SCORE_CREATE FS_BARRACKS",
        build_cost: 500,
        build_time_sec: 10.0,
        max_health: 1000.0,
        is_infantry: false,
        is_vehicle: false,
        is_aircraft: false,
        is_structure: true,
        is_transport: false,
        is_salvager: false,
    },
];

/// True when KindOf residual string contains token (whitespace-delimited).
pub fn kindof_has_token(kind_of: &str, token: &str) -> bool {
    kind_of.split_whitespace().any(|t| t == token)
}

/// Lookup Object KindOf residual pack by object template name.
pub fn object_kindof_residual_pack_wave103(
    object_name: &str,
) -> Option<&'static ObjectKindOfResidualPack> {
    OBJECT_KINDOF_RESIDUAL_PACKS_WAVE103
        .iter()
        .find(|p| p.object_name == object_name)
}

/// Wave 103 honesty: Object residual KindOf packs for more unit types.
///
/// Freezes KindOf token residual + BuildCost / BuildTime / MaxHealth residual
/// for common infantry / vehicle / aircraft / structure templates.
/// Fail-closed: not full ThingTemplate KindOf bit matrix / live INI parse.
pub fn honesty_object_kindof_residual_pack_wave103() -> bool {
    if OBJECT_KINDOF_RESIDUAL_PACKS_WAVE103.len() < 16 {
        return false;
    }
    // Unique object names.
    let mut names: Vec<&str> = OBJECT_KINDOF_RESIDUAL_PACKS_WAVE103
        .iter()
        .map(|p| p.object_name)
        .collect();
    names.sort_unstable();
    if names.windows(2).any(|w| w[0] == w[1]) {
        return false;
    }
    // Shared token residual for combat units (SELECTABLE present).
    let selectable_ok = OBJECT_KINDOF_RESIDUAL_PACKS_WAVE103
        .iter()
        .all(|p| kindof_has_token(p.kind_of, "SELECTABLE"));
    // Classification residual flags match KindOf tokens.
    let flags_ok = OBJECT_KINDOF_RESIDUAL_PACKS_WAVE103.iter().all(|p| {
        let inf = kindof_has_token(p.kind_of, "INFANTRY") == p.is_infantry;
        let veh = kindof_has_token(p.kind_of, "VEHICLE") == p.is_vehicle;
        let air = kindof_has_token(p.kind_of, "AIRCRAFT") == p.is_aircraft;
        let stru = kindof_has_token(p.kind_of, "STRUCTURE") == p.is_structure;
        let transport = kindof_has_token(p.kind_of, "TRANSPORT") == p.is_transport;
        let salvager = kindof_has_token(p.kind_of, "SALVAGER") == p.is_salvager;
        inf && veh && air && stru && transport && salvager
            && p.build_cost > 0
            && p.build_time_sec > 0.0
            && p.max_health > 0.0
    });
    // Anchor residual packs (retail Object.ini).
    let ranger = object_kindof_residual_pack_wave103("AmericaInfantryRanger");
    let crusader = object_kindof_residual_pack_wave103("AmericaTankCrusader");
    let raptor = object_kindof_residual_pack_wave103("AmericaJetRaptor");
    let overlord = object_kindof_residual_pack_wave103("ChinaTankOverlord");
    let technical = object_kindof_residual_pack_wave103("GLAVehicleTechnical");
    let cc = object_kindof_residual_pack_wave103("AmericaCommandCenter");
    let helix = object_kindof_residual_pack_wave103("ChinaVehicleHelix");
    let anchors_ok = ranger
        .map(|p| {
            p.is_infantry
                && kindof_has_token(p.kind_of, "CAN_RAPPEL")
                && p.build_cost == 225
                && (p.max_health - 180.0).abs() < 0.01
        })
        .unwrap_or(false)
        && crusader
            .map(|p| {
                p.is_vehicle
                    && !p.is_aircraft
                    && p.build_cost == 900
                    && (p.max_health - 480.0).abs() < 0.01
            })
            .unwrap_or(false)
        && raptor
            .map(|p| {
                p.is_aircraft
                    && p.is_vehicle
                    && p.build_cost == 1400
                    && (p.max_health - 160.0).abs() < 0.01
            })
            .unwrap_or(false)
        && overlord
            .map(|p| {
                p.is_vehicle
                    && kindof_has_token(p.kind_of, "HUGE_VEHICLE")
                    && p.build_cost == 2000
                    && (p.max_health - 1100.0).abs() < 0.01
            })
            .unwrap_or(false)
        && technical
            .map(|p| {
                p.is_transport
                    && p.is_salvager
                    && kindof_has_token(p.kind_of, "WEAPON_SALVAGER")
                    && p.build_cost == 500
            })
            .unwrap_or(false)
        && cc
            .map(|p| {
                p.is_structure
                    && kindof_has_token(p.kind_of, "COMMANDCENTER")
                    && kindof_has_token(p.kind_of, "FS_FACTORY")
                    && p.build_cost == 2000
                    && (p.max_health - 5000.0).abs() < 0.01
            })
            .unwrap_or(false)
        && helix
            .map(|p| {
                p.is_aircraft
                    && p.is_transport
                    && kindof_has_token(p.kind_of, "PRODUCED_AT_HELIPAD")
                    && kindof_has_token(p.kind_of, "HUGE_VEHICLE")
            })
            .unwrap_or(false);
    selectable_ok && flags_ok && anchors_ok
}

// ---------------------------------------------------------------------------
// Combined Wave 103 residual pack
// ---------------------------------------------------------------------------

/// Combined Wave 103 game-logic residual honesty pack.
///
/// Weapon deepen + armor expand + locomotor expand + Superweapon SpecialPower
/// residual deepen + Object KindOf residual packs.
/// Fail-closed: not full INI archive / shell playable_claim / network.
pub fn honesty_game_logic_residual_pack_wave103() -> bool {
    honesty_weapon_store_deepen_residual_wave103()
        && honesty_armor_residual_expand_wave103()
        && honesty_locomotor_residual_expand_wave103()
        && honesty_special_power_superweapon_residual_deepen_wave103()
        && honesty_object_kindof_residual_pack_wave103()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_residual_pack_honesty_wave103() {
        assert!(honesty_weapon_store_deepen_residual_wave103());
    }

    #[test]
    fn armor_residual_pack_honesty_wave103() {
        assert!(honesty_armor_residual_expand_wave103());
    }

    #[test]
    fn locomotor_residual_pack_honesty_wave103() {
        assert!(honesty_locomotor_residual_expand_wave103());
    }

    #[test]
    fn special_power_residual_pack_honesty_wave103() {
        assert!(honesty_special_power_superweapon_residual_deepen_wave103());
        assert_eq!(
            superweapon_special_power_row_wave103("SuperweaponMOAB")
                .map(|(_, e, ms)| (e, ms)),
            Some(("SPECIAL_DAISY_CUTTER", 360_000))
        );
        assert_eq!(
            superweapon_special_power_row_wave103("SpecialPowerSpySatellite")
                .map(|(_, e, ms)| (e, ms)),
            Some(("SPECIAL_SPY_SATELLITE", 60_000))
        );
    }

    #[test]
    fn object_kindof_residual_pack_honesty_wave103() {
        assert!(honesty_object_kindof_residual_pack_wave103());
        let ranger = object_kindof_residual_pack_wave103("AmericaInfantryRanger").unwrap();
        assert!(kindof_has_token(ranger.kind_of, "INFANTRY"));
        assert!(kindof_has_token(ranger.kind_of, "CAN_RAPPEL"));
        let overlord = object_kindof_residual_pack_wave103("ChinaTankOverlord").unwrap();
        assert!((overlord.max_health - 1100.0).abs() < 0.01);
    }

    #[test]
    fn game_logic_residual_pack_honesty_wave103() {
        assert!(honesty_game_logic_residual_pack_wave103());
    }
}
