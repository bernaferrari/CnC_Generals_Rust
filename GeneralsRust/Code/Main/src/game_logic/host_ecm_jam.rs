//! Host China ECM Tank / jammer residual (weapon jam aura).
//!
//! Residual slice (playability):
//! - ChinaTankECM / *TankECM / FrequencyJammer residual sources:
//!   continuous enemy-weapon jam field inspired by retail ECMTankVehicleDisabler
//!   (SUBDUAL_VEHICLE → DISABLED_SUBDUED cannot fire) + ECMTankMissileJammer
//!   FireWeaponUpdate pulse (PrimaryDamageRadius=150).
//! - Enemies (and neutrals) with weapons inside the radius get `weapons_jammed`
//!   residual and cannot fire until they leave the field or the jammer dies.
//!
//! Fail-closed honesty:
//! - Not full subdual damage accumulate / SubdualDamageHelper heal drain
//! - Not full laser attach / ECMDisableStream / FireWeaponUpdate exclusive delay
//! - Not full missile projectile_now_jammed scatter path (separate residual)
//! - Not full ally relationship / underpower / DISABLED_SUBDUED FX tint matrix
//! - Not network jam replication (network deferred)

/// Retail ECMTankMissileJammer PrimaryDamageRadius residual (= 150).
/// Also covers residual vehicle-disabler engagement band (AttackRange=200 fail-closed).
pub const HOST_ECM_JAM_RADIUS: f32 = 150.0;

/// Whether template is a residual ECM tank / frequency jammer source.
///
/// Fail-closed: name-based residual (not full INI FireWeaponUpdate / WeaponSet matrix).
pub fn is_ecm_jammer(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    // ChinaTankECM, Tank_ChinaTankECM, Nuke_ChinaTankECM, Infa_ChinaTankECM, …
    if n.contains("tankecm") || n.contains("ecmtank") {
        return true;
    }
    // FrequencyJammer voice-named residual / cinematic variants.
    if n.contains("frequencyjammer") || n.contains("missilejammer") {
        return true;
    }
    // Explicit residual test / shorthand names.
    if n == "testecmtank" || (n.ends_with("ecm") && n.contains("tank")) {
        return true;
    }
    false
}

/// Whether residual target can have weapons jammed by an ECM field.
///
/// Retail: vehicle disabler hits ground vehicles; jammer pulse affects ENEMIES/NEUTRALS.
/// Residual: any alive armed non-structure enemy/neutral (not self, not under construction).
pub fn is_legal_ecm_jam_target(
    is_structure: bool,
    is_alive: bool,
    enemy_or_neutral: bool,
    is_self: bool,
    under_construction: bool,
    has_weapon: bool,
) -> bool {
    !is_structure
        && is_alive
        && enemy_or_neutral
        && !is_self
        && !under_construction
        && has_weapon
}

/// 2D distance check residual (C++ FROM_CENTER_2D).
pub fn in_ecm_jam_radius_2d(jammer_pos: (f32, f32), target_pos: (f32, f32), radius: f32) -> bool {
    let dx = jammer_pos.0 - target_pos.0;
    let dy = jammer_pos.1 - target_pos.1;
    dx * dx + dy * dy <= radius * radius
}

/// True when jammer team vs target team is residual-hostile (enemy) or Neutral victim.
///
/// Retail ECMTankMissileJammer: RadiusDamageAffects = ENEMIES NEUTRALS.
pub fn is_ecm_hostile_team(jammer_team_is_neutral: bool, same_team: bool, target_is_neutral: bool) -> bool {
    if jammer_team_is_neutral {
        // Neutral jammer residual does not jam anyone (fail-closed).
        return false;
    }
    !same_team || target_is_neutral
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecm_jammer_name_matrix() {
        assert!(is_ecm_jammer("ChinaTankECM"));
        assert!(is_ecm_jammer("Tank_ChinaTankECM"));
        assert!(is_ecm_jammer("Nuke_ChinaTankECM"));
        assert!(is_ecm_jammer("Infa_ChinaTankECM"));
        assert!(is_ecm_jammer("TestECMTank"));
        assert!(is_ecm_jammer("FrequencyJammer"));
        assert!(is_ecm_jammer("AmericaMissileJammer"));
        assert!(!is_ecm_jammer("ChinaTankBattleMaster"));
        assert!(!is_ecm_jammer("USA_Ranger"));
        assert!(!is_ecm_jammer("TestTank"));
        assert!(!is_ecm_jammer("ChinaSpeakerTower"));
        assert!(!is_ecm_jammer("AmericaVehicleMedic"));
    }

    #[test]
    fn legal_ecm_jam_target_matrix() {
        // structure, alive, enemy_or_neutral, is_self, under_construction, has_weapon
        assert!(is_legal_ecm_jam_target(false, true, true, false, false, true));
        assert!(!is_legal_ecm_jam_target(true, true, true, false, false, true));
        assert!(!is_legal_ecm_jam_target(false, false, true, false, false, true));
        assert!(!is_legal_ecm_jam_target(false, true, false, false, false, true));
        assert!(!is_legal_ecm_jam_target(false, true, true, true, false, true));
        assert!(!is_legal_ecm_jam_target(false, true, true, false, true, true));
        assert!(!is_legal_ecm_jam_target(false, true, true, false, false, false));
    }

    #[test]
    fn ecm_radius_and_team_filters() {
        assert!(HOST_ECM_JAM_RADIUS > 0.0);
        assert!(in_ecm_jam_radius_2d((0.0, 0.0), (50.0, 0.0), 150.0));
        assert!(!in_ecm_jam_radius_2d((0.0, 0.0), (200.0, 0.0), 150.0));
        assert!(is_ecm_hostile_team(false, false, false)); // enemy
        assert!(is_ecm_hostile_team(false, false, true)); // neutral victim
        assert!(!is_ecm_hostile_team(false, true, false)); // same team ally
        assert!(!is_ecm_hostile_team(true, false, false)); // neutral jammer
    }
}
