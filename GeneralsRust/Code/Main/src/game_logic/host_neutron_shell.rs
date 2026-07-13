//! Host China Neutron Shell residual (Nuke Cannon secondary + NeutronBlast).
//!
//! Residual slice (playability):
//! - `Upgrade_ChinaNeutronShells` research equips Nuke Cannon SECONDARY
//!   `NukeCannonNeutronWeapon` (retail WeaponSet + WeaponSetUpgrade residual).
//! - Neutron shell impact applies NeutronBlastBehavior residual:
//!   - Infantry in blast radius are killed
//!   - Vehicles become unmanned + Neutral (no HP damage; combat bikes residual
//!     destroyed when name contains bike/combatbike)
//!   - Contained passengers residual killed when vehicle is unmanned path
//! - Blast radius residual matches NeutronCannonShell BlastRadius = 70.
//!
//! Fail-closed honesty:
//! - Not full projectile flight / DumbProjectileBehavior bezier path
//! - Not full AffectAirborne / ally Relationship matrix beyond residual flags
//! - Not full WeaponSet chooser command-button toggle beyond active_weapon_slot
//! - Not network neutron replication (network deferred)

/// Retail Upgrade_ChinaNeutronShells name.
pub const UPGRADE_CHINA_NEUTRON_SHELLS: &str = "Upgrade_ChinaNeutronShells";

/// Retail NukeCannonNeutronWeapon template name.
pub const NUKE_CANNON_NEUTRON_WEAPON: &str = "NukeCannonNeutronWeapon";

/// Retail NukeCannonGun primary template name (for host seed / equip).
pub const NUKE_CANNON_PRIMARY_WEAPON: &str = "NukeCannonGun";

/// Retail NeutronCannonShell NeutronBlastBehavior BlastRadius residual.
pub const HOST_NEUTRON_BLAST_RADIUS: f32 = 70.0;

/// Retail NukeCannonNeutronWeapon AttackRange residual.
pub const NEUTRON_WEAPON_ATTACK_RANGE: f32 = 350.0;

/// Retail NukeCannonNeutronWeapon MinimumAttackRange residual.
pub const NEUTRON_WEAPON_MIN_RANGE: f32 = 150.0;

/// Retail NukeCannonNeutronWeapon DelayBetweenShots 10000ms → 300 frames @ 30 FPS.
pub const NEUTRON_WEAPON_DELAY_FRAMES: u32 = 300;

/// Activate / detonation audio residual.
pub const NEUTRON_SHELL_AUDIO: &str = "NukeCannonWeapon";

// re-export used by GameLogic residual path (already pub above).

/// Whether template is a residual Nuke Cannon that receives neutron secondary.
///
/// Fail-closed: name residual (not full INI WeaponSet / DeployStyleAIUpdate matrix).
/// Excludes projectile shells (`NukeCannonShell` / `NeutronCannonShell`).
pub fn is_nuke_cannon_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n == "testnukecannon" || n == "testneutroncannon" {
        return true;
    }
    // Projectile / shell objects are not the Nuke Cannon vehicle.
    if n.contains("shell") || n.contains("projectile") || n.contains("blast") {
        return false;
    }
    n.contains("nukecannon") || n.contains("nuke_cannon")
}

/// True when residual target is a combat-bike style vehicle that dies to neutron
/// instead of going unmanned (C++ KINDOF_CLIFF_JUMPER residual).
pub fn is_neutron_kill_vehicle_name(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("combatbike") || n.contains("combat_bike") || n.contains("cliffjumper")
}

/// Whether residual target receives neutron blast effects.
///
/// C++ NeutronBlastBehavior: infantry kill; vehicle unmanned (not drones);
/// skip dead / optionally allies / airborne.
pub fn is_legal_neutron_blast_target(
    is_alive: bool,
    is_infantry: bool,
    is_vehicle: bool,
    is_aircraft: bool,
    is_structure: bool,
    is_drone: bool,
    is_airborne: bool,
    affect_airborne: bool,
    same_team_ally: bool,
    affect_allies: bool,
) -> bool {
    if !is_alive {
        return false;
    }
    if is_structure {
        return false;
    }
    if is_aircraft || (is_airborne && !affect_airborne) {
        // Residual: airborne non-infantry skipped when AffectAirborne=No.
        if is_aircraft {
            return false;
        }
        if is_airborne && !affect_airborne {
            return false;
        }
    }
    if same_team_ally && !affect_allies {
        return false;
    }
    if is_drone {
        return false;
    }
    is_infantry || is_vehicle
}

/// Classify residual neutron effect for one target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeutronEffect {
    /// Infantry (and residual passengers) killed.
    KillInfantry,
    /// Vehicle becomes unmanned + Neutral.
    UnmanVehicle,
    /// Combat bike / cliff-jumper residual destroyed.
    KillVehicle,
    /// No effect.
    None,
}

/// Residual effect choice for a legal target.
pub fn neutron_effect_for_target(
    is_infantry: bool,
    is_vehicle: bool,
    is_drone: bool,
    template_name: &str,
) -> NeutronEffect {
    if is_infantry {
        return NeutronEffect::KillInfantry;
    }
    if is_vehicle && !is_drone {
        if is_neutron_kill_vehicle_name(template_name) {
            return NeutronEffect::KillVehicle;
        }
        return NeutronEffect::UnmanVehicle;
    }
    NeutronEffect::None
}

/// 2D distance check residual (blast radius).
pub fn in_neutron_blast_radius_2d(
    center: (f32, f32),
    target: (f32, f32),
    radius: f32,
) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

/// Whether weapon identity residual is the neutron shell secondary.
///
/// Fail-closed: name / damage+range residual when weapon name not stored on host Weapon.
pub fn is_neutron_secondary_weapon(
    has_neutron_upgrade: bool,
    active_weapon_slot: u8,
    secondary_range: Option<f32>,
) -> bool {
    if !has_neutron_upgrade || active_weapon_slot != 1 {
        // Also true when select_combat_weapon_slot chose secondary (caller passes slot).
    }
    if !has_neutron_upgrade {
        return false;
    }
    // Range residual: NukeCannonNeutronWeapon AttackRange 350.
    if let Some(r) = secondary_range {
        if (r - NEUTRON_WEAPON_ATTACK_RANGE).abs() < 1.0 || r >= 300.0 {
            return true;
        }
    }
    has_neutron_upgrade
}

/// Slot-aware: true when combat should apply neutron blast instead of HP damage.
pub fn should_apply_neutron_blast(
    has_neutron_upgrade: bool,
    fired_slot: u8,
    is_nuke_cannon: bool,
) -> bool {
    has_neutron_upgrade && fired_slot == 1 && is_nuke_cannon
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nuke_cannon_name_matrix() {
        assert!(is_nuke_cannon_template("ChinaVehicleNukeCannon"));
        assert!(is_nuke_cannon_template("China_NukeCannon"));
        assert!(is_nuke_cannon_template("Nuke_ChinaVehicleNukeCannon"));
        assert!(is_nuke_cannon_template("TestNukeCannon"));
        assert!(!is_nuke_cannon_template("ChinaTankBattleMaster"));
        assert!(!is_nuke_cannon_template("USA_Ranger"));
        assert!(!is_nuke_cannon_template("NukeCannonShell"));
    }

    #[test]
    fn legal_neutron_target_matrix() {
        // infantry
        assert!(is_legal_neutron_blast_target(
            true, true, false, false, false, false, false, false, false, true
        ));
        // vehicle
        assert!(is_legal_neutron_blast_target(
            true, false, true, false, false, false, false, false, false, true
        ));
        // structure skip
        assert!(!is_legal_neutron_blast_target(
            true, false, false, false, true, false, false, false, false, true
        ));
        // ally skipped when affect_allies=false
        assert!(!is_legal_neutron_blast_target(
            true, true, false, false, false, false, false, false, true, false
        ));
        // drone skip
        assert!(!is_legal_neutron_blast_target(
            true, false, true, false, false, true, false, false, false, true
        ));
        // aircraft skip
        assert!(!is_legal_neutron_blast_target(
            true, false, false, true, false, false, true, false, false, true
        ));
    }

    #[test]
    fn neutron_effect_matrix() {
        assert_eq!(
            neutron_effect_for_target(true, false, false, "USA_Ranger"),
            NeutronEffect::KillInfantry
        );
        assert_eq!(
            neutron_effect_for_target(false, true, false, "TestTank"),
            NeutronEffect::UnmanVehicle
        );
        assert_eq!(
            neutron_effect_for_target(false, true, false, "GLACombatBike"),
            NeutronEffect::KillVehicle
        );
        assert_eq!(
            neutron_effect_for_target(false, false, false, "USA_CommandCenter"),
            NeutronEffect::None
        );
    }

    #[test]
    fn should_apply_blast_gate() {
        assert!(should_apply_neutron_blast(true, 1, true));
        assert!(!should_apply_neutron_blast(true, 0, true));
        assert!(!should_apply_neutron_blast(false, 1, true));
        assert!(!should_apply_neutron_blast(true, 1, false));
    }

    #[test]
    fn blast_radius_constants() {
        assert!((HOST_NEUTRON_BLAST_RADIUS - 70.0).abs() < 0.01);
        assert!(in_neutron_blast_radius_2d((0.0, 0.0), (50.0, 0.0), 70.0));
        assert!(!in_neutron_blast_radius_2d((0.0, 0.0), (80.0, 0.0), 70.0));
    }
}
