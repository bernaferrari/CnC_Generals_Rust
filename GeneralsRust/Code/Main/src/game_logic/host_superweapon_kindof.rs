//! Wave 80: Object KindOf residual packs for common superweapon buildings.
//!
//! Retail `FactionBuilding.ini` KindOf / BuildCost / BuildTime / MaxHealth /
//! EnergyProduction residual for:
//! - AmericaParticleCannonUplink
//! - GLAScudStorm
//! - ChinaNuclearMissileLauncher
//!
//! Shared residual tokens: STRUCTURE SELECTABLE IMMOBILE CAPTURABLE
//! FS_TECHNOLOGY FS_SUPERWEAPON MP_COUNT_FOR_VICTORY (+ faction SCORE variants).
//!
//! Fail-closed:
//! - Not full ThingTemplate KindOf bit matrix / INI parse
//! - Not live MaxSimultaneousOfType superweapon restriction UI
//! - Shell `playable_claim` stays false; network deferred

use serde::{Deserialize, Serialize};

/// Retail superweapon building BuildCost residual (all three baseline SW).
pub const SUPERWEAPON_BUILD_COST: i32 = 5000;
/// Retail superweapon building BuildTime residual (seconds).
pub const SUPERWEAPON_BUILD_TIME_SEC: f32 = 60.0;
/// Retail superweapon StructureBody MaxHealth residual.
pub const SUPERWEAPON_MAX_HEALTH: f32 = 4000.0;
/// Retail EnergyProduction residual for powered superweapons (Particle / Nuke).
pub const SUPERWEAPON_ENERGY_DRAIN: i32 = -10;
/// Retail EnergyProduction residual for unpowered Scud Storm.
pub const SCUD_STORM_ENERGY_PRODUCTION: i32 = 0;

/// Retail MaxSimultaneousLinkKey residual string ("Superweapon").
/// Counts all FS_SUPERWEAPON buildings **and** GLA Scud Storm rebuild holes as one type.
pub const SUPERWEAPON_LINK_KEY: &str = "Superweapon";
/// Retail multiplayer DeterminedBySuperweaponRestriction residual when limited.
pub const SUPERWEAPON_MAX_SIMULTANEOUS_WHEN_LIMITED: u32 = 1;

/// Object template residual names.
pub const AMERICA_PARTICLE_CANNON_UPLINK: &str = "AmericaParticleCannonUplink";
pub const GLA_SCUD_STORM: &str = "GLAScudStorm";
pub const CHINA_NUCLEAR_MISSILE_LAUNCHER: &str = "ChinaNuclearMissileLauncher";

/// DisplayName residual keys.
pub const PARTICLE_CANNON_DISPLAY_NAME: &str = "OBJECT:ParticleCannon";
pub const NUCLEAR_MISSILE_DISPLAY_NAME: &str = "OBJECT:NuclearMissile";

/// CommandSet residual names.
pub const PARTICLE_CANNON_COMMAND_SET: &str = "AmericaParticleUplinkCannonCommandSet";
pub const SCUD_STORM_COMMAND_SET: &str = "GLAScudStormCommandSet";
pub const NUCLEAR_MISSILE_COMMAND_SET: &str = "ChinaNuclearMissileCommandSet";

/// Shared KindOf residual tokens present on all three superweapon buildings.
pub const SUPERWEAPON_SHARED_KINDOF_TOKENS: &[&str] = &[
    "PRELOAD",
    "STRUCTURE",
    "SELECTABLE",
    "IMMOBILE",
    "CAPTURABLE",
    "FS_TECHNOLOGY",
    "MP_COUNT_FOR_VICTORY",
    "FS_SUPERWEAPON",
];

/// KindOf residual string for AmericaParticleCannonUplink.
pub const PARTICLE_CANNON_KINDOF: &str = "PRELOAD STRUCTURE SELECTABLE IMMOBILE SCORE CAPTURABLE FS_TECHNOLOGY POWERED MP_COUNT_FOR_VICTORY FS_SUPERWEAPON";
/// KindOf residual string for GLAScudStorm (SCORE_CREATE, no POWERED).
pub const SCUD_STORM_KINDOF: &str = "PRELOAD STRUCTURE SELECTABLE IMMOBILE CAPTURABLE FS_TECHNOLOGY MP_COUNT_FOR_VICTORY SCORE_CREATE FS_SUPERWEAPON";
/// KindOf residual string for ChinaNuclearMissileLauncher.
pub const NUCLEAR_MISSILE_KINDOF: &str = "PRELOAD STRUCTURE SELECTABLE IMMOBILE SCORE CAPTURABLE FS_TECHNOLOGY POWERED MP_COUNT_FOR_VICTORY FS_SUPERWEAPON";

/// Common superweapon building residual pack.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SuperweaponBuildingKindOfResidual {
    pub object_name: &'static str,
    pub kind_of: &'static str,
    pub build_cost: i32,
    pub build_time_sec: f32,
    pub max_health: f32,
    pub energy_production: i32,
    pub command_set: &'static str,
    pub is_powered: bool,
    pub has_score: bool,
    pub has_score_create: bool,
}

/// America Particle Uplink residual pack.
pub fn particle_cannon_kindof_residual() -> SuperweaponBuildingKindOfResidual {
    SuperweaponBuildingKindOfResidual {
        object_name: AMERICA_PARTICLE_CANNON_UPLINK,
        kind_of: PARTICLE_CANNON_KINDOF,
        build_cost: SUPERWEAPON_BUILD_COST,
        build_time_sec: SUPERWEAPON_BUILD_TIME_SEC,
        max_health: SUPERWEAPON_MAX_HEALTH,
        energy_production: SUPERWEAPON_ENERGY_DRAIN,
        command_set: PARTICLE_CANNON_COMMAND_SET,
        is_powered: true,
        has_score: true,
        has_score_create: false,
    }
}

/// GLA Scud Storm residual pack.
pub fn scud_storm_kindof_residual() -> SuperweaponBuildingKindOfResidual {
    SuperweaponBuildingKindOfResidual {
        object_name: GLA_SCUD_STORM,
        kind_of: SCUD_STORM_KINDOF,
        build_cost: SUPERWEAPON_BUILD_COST,
        build_time_sec: SUPERWEAPON_BUILD_TIME_SEC,
        max_health: SUPERWEAPON_MAX_HEALTH,
        energy_production: SCUD_STORM_ENERGY_PRODUCTION,
        command_set: SCUD_STORM_COMMAND_SET,
        is_powered: false,
        has_score: false,
        has_score_create: true,
    }
}

/// China Nuclear Missile Launcher residual pack.
pub fn nuclear_missile_kindof_residual() -> SuperweaponBuildingKindOfResidual {
    SuperweaponBuildingKindOfResidual {
        object_name: CHINA_NUCLEAR_MISSILE_LAUNCHER,
        kind_of: NUCLEAR_MISSILE_KINDOF,
        build_cost: SUPERWEAPON_BUILD_COST,
        build_time_sec: SUPERWEAPON_BUILD_TIME_SEC,
        max_health: SUPERWEAPON_MAX_HEALTH,
        energy_production: SUPERWEAPON_ENERGY_DRAIN,
        command_set: NUCLEAR_MISSILE_COMMAND_SET,
        is_powered: true,
        has_score: true,
        has_score_create: false,
    }
}

/// All three common superweapon building residual packs.
pub fn common_superweapon_kindof_packs() -> [SuperweaponBuildingKindOfResidual; 3] {
    [
        particle_cannon_kindof_residual(),
        scud_storm_kindof_residual(),
        nuclear_missile_kindof_residual(),
    ]
}

/// True when KindOf residual string contains every shared superweapon token.
pub fn kindof_has_shared_superweapon_tokens(kind_of: &str) -> bool {
    SUPERWEAPON_SHARED_KINDOF_TOKENS
        .iter()
        .all(|tok| kind_of.split_whitespace().any(|t| t == *tok))
}

/// Wave 80 honesty: common superweapon Object KindOf residual pack.
///
/// Fail-closed: not full ThingTemplate KindOf bit matrix / live INI parse.
pub fn honesty_superweapon_kindof_residual_pack_wave80() -> bool {
    let puc = particle_cannon_kindof_residual();
    let scud = scud_storm_kindof_residual();
    let nuke = nuclear_missile_kindof_residual();
    let packs = common_superweapon_kindof_packs();

    SUPERWEAPON_BUILD_COST == 5000
        && (SUPERWEAPON_BUILD_TIME_SEC - 60.0).abs() < 0.01
        && (SUPERWEAPON_MAX_HEALTH - 4000.0).abs() < 0.01
        && SUPERWEAPON_ENERGY_DRAIN == -10
        && SCUD_STORM_ENERGY_PRODUCTION == 0
        && AMERICA_PARTICLE_CANNON_UPLINK == "AmericaParticleCannonUplink"
        && GLA_SCUD_STORM == "GLAScudStorm"
        && CHINA_NUCLEAR_MISSILE_LAUNCHER == "ChinaNuclearMissileLauncher"
        && PARTICLE_CANNON_DISPLAY_NAME == "OBJECT:ParticleCannon"
        && NUCLEAR_MISSILE_DISPLAY_NAME == "OBJECT:NuclearMissile"
        && packs.len() == 3
        // Shared FS_SUPERWEAPON / STRUCTURE residual tokens.
        && packs
            .iter()
            .all(|p| kindof_has_shared_superweapon_tokens(p.kind_of))
        // Particle: POWERED + SCORE + energy drain.
        && puc.is_powered
        && puc.has_score
        && !puc.has_score_create
        && puc.energy_production == -10
        && puc.kind_of.contains("POWERED")
        && puc.kind_of.contains("FS_SUPERWEAPON")
        && puc.kind_of.split_whitespace().any(|t| t == "SCORE")
        && puc.command_set == "AmericaParticleUplinkCannonCommandSet"
        // Scud: SCORE_CREATE, no POWERED, energy 0.
        && !scud.is_powered
        && scud.has_score_create
        && !scud.has_score
        && scud.energy_production == 0
        && scud.kind_of.contains("SCORE_CREATE")
        && !scud.kind_of.split_whitespace().any(|t| t == "POWERED")
        && scud.command_set == "GLAScudStormCommandSet"
        // Nuke: POWERED + SCORE + energy drain (matches Particle KindOf shape).
        && nuke.is_powered
        && nuke.has_score
        && nuke.energy_production == -10
        && nuke.kind_of == NUCLEAR_MISSILE_KINDOF
        && nuke.kind_of == PARTICLE_CANNON_KINDOF // same KindOf residual string
        && nuke.command_set == "ChinaNuclearMissileCommandSet"
        // Shared economy residual.
        && packs.iter().all(|p| {
            p.build_cost == SUPERWEAPON_BUILD_COST
                && (p.build_time_sec - SUPERWEAPON_BUILD_TIME_SEC).abs() < 0.01
                && (p.max_health - SUPERWEAPON_MAX_HEALTH).abs() < 0.01
        })
}

/// True when template is counted under MaxSimultaneousLinkKey=Superweapon residual.
///
/// Includes baseline PUC / Nuke / Scud + rebuild-hole residual name tokens.
pub fn is_superweapon_link_key_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.contains("rebuildhole") && (n.contains("scud") || n.contains("superweapon")) {
        return true;
    }
    if n.contains("particlecannon")
        || n.contains("particleuplink")
        || n.contains("scudstorm")
        || n.contains("nuclearmissile")
        || n.contains("nuclear_missile")
    {
        return true;
    }
    // Exact retail names residual.
    matches!(
        template_name,
        AMERICA_PARTICLE_CANNON_UPLINK
            | GLA_SCUD_STORM
            | CHINA_NUCLEAR_MISSILE_LAUNCHER
            | "AmericaParticleUplinkCannon"
            | "SupW_AmericaParticleCannonUplink"
            | "Lazr_AmericaParticleCannonUplink"
            | "Nuke_ChinaNuclearMissileLauncher"
            | "Chem_GLAScudStorm"
    )
}

/// C++ MaxSimultaneousOfType=DeterminedBySuperweaponRestriction residual.
///
/// When `limit_superweapons` is false (default skirmish/SP residual), unlimited.
/// When true (MP option residual), max simultaneous = 1 across Superweapon link key.
pub fn superweapon_max_simultaneous_allowed(limit_superweapons: bool) -> Option<u32> {
    if limit_superweapons {
        Some(SUPERWEAPON_MAX_SIMULTANEOUS_WHEN_LIMITED)
    } else {
        None
    }
}

/// Honesty pack residual for MaxSimultaneous link key.

/// C++ EnergyProduction residual for a superweapon template name.
///
/// Returns `Some(energy)` where negative values drain power (PUC/Nuke **-10**),
/// Scud Storm **0**. Non-superweapon templates return `None`.

/// Map a superweapon structure template to its host SpecialPowerType residual.
///
/// Used by C++ SpecialPowerModule::onSpecialPowerCreation / startPowerRecharge
/// when a SW structure finishes construction (or is map-placed fully built).
pub fn special_power_for_superweapon_structure(
    template_name: &str,
) -> Option<crate::command_system::SpecialPowerType> {
    use crate::command_system::SpecialPowerType as P;
    if !is_superweapon_link_key_template(template_name) {
        return None;
    }
    let n = template_name.to_ascii_lowercase();
    if n.contains("rebuildhole") {
        return None;
    }
    if n.contains("particlecannon") || n.contains("particleuplink") {
        return Some(P::ParticleCannon);
    }
    if n.contains("scudstorm") || n.contains("scud_storm") {
        return Some(P::ScudStorm);
    }
    if n.contains("nuclearmissile")
        || n.contains("nuclear_missile")
        || (n.contains("nuke") && n.contains("launcher"))
    {
        return Some(P::NuclearMissile);
    }
    None
}

pub fn superweapon_energy_production_for_template(template_name: &str) -> Option<i32> {
    if !is_superweapon_link_key_template(template_name) {
        return None;
    }
    let n = template_name.to_ascii_lowercase();
    // Rebuild holes do not drain (not live SW structure residual).
    if n.contains("rebuildhole") {
        return Some(0);
    }
    if n.contains("scudstorm") || n.contains("scud_storm") {
        return Some(SCUD_STORM_ENERGY_PRODUCTION);
    }
    // Particle Cannon / Nuclear Missile powered SW residual.
    Some(SUPERWEAPON_ENERGY_DRAIN)
}

/// Map EnergyProduction residual onto Object power_provided / power_consumed.
///
/// C++ negative EnergyProduction → power_consumed = abs(energy); positive → power_provided.
pub fn apply_energy_production_to_power(energy_production: i32) -> (i32, i32) {
    if energy_production >= 0 {
        (energy_production, 0)
    } else {
        (0, energy_production.abs())
    }
}

pub fn honesty_superweapon_max_simultaneous_residual_pack() -> bool {
    SUPERWEAPON_LINK_KEY == "Superweapon"
        && SUPERWEAPON_MAX_SIMULTANEOUS_WHEN_LIMITED == 1
        && is_superweapon_link_key_template(AMERICA_PARTICLE_CANNON_UPLINK)
        && is_superweapon_link_key_template(GLA_SCUD_STORM)
        && is_superweapon_link_key_template(CHINA_NUCLEAR_MISSILE_LAUNCHER)
        && is_superweapon_link_key_template("GLAScudStormRebuildHole")
        && !is_superweapon_link_key_template("AmericaCommandCenter")
        && superweapon_max_simultaneous_allowed(false).is_none()
        && superweapon_max_simultaneous_allowed(true) == Some(1)
        && superweapon_energy_production_for_template(AMERICA_PARTICLE_CANNON_UPLINK)
            == Some(SUPERWEAPON_ENERGY_DRAIN)
        && superweapon_energy_production_for_template(CHINA_NUCLEAR_MISSILE_LAUNCHER)
            == Some(SUPERWEAPON_ENERGY_DRAIN)
        && superweapon_energy_production_for_template(GLA_SCUD_STORM)
            == Some(SCUD_STORM_ENERGY_PRODUCTION)
        && apply_energy_production_to_power(SUPERWEAPON_ENERGY_DRAIN) == (0, 10)
        && apply_energy_production_to_power(0) == (0, 0)
        && special_power_for_superweapon_structure(AMERICA_PARTICLE_CANNON_UPLINK)
            == Some(crate::command_system::SpecialPowerType::ParticleCannon)
        && special_power_for_superweapon_structure(GLA_SCUD_STORM)
            == Some(crate::command_system::SpecialPowerType::ScudStorm)
        && special_power_for_superweapon_structure(CHINA_NUCLEAR_MISSILE_LAUNCHER)
            == Some(crate::command_system::SpecialPowerType::NuclearMissile)
        && special_power_for_superweapon_structure("GLAScudStormRebuildHole").is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superweapon_kindof_residual_pack_wave80_honesty() {
        assert!(honesty_superweapon_kindof_residual_pack_wave80());
        assert!(kindof_has_shared_superweapon_tokens(SCUD_STORM_KINDOF));
        assert!(!kindof_has_shared_superweapon_tokens(
            "STRUCTURE SELECTABLE"
        ));
    }
}
