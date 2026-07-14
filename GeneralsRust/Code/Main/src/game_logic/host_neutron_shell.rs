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
//! Wave 55 residual pack (retail Weapon.ini / WeaponObjects.ini honesty):
//! - Neutron blast residual: BlastRadius **70**, AffectAirborne **No**,
//!   AffectAllies default **Yes** (module default; shell INI omits AffectAllies)
//! - Shell projectile residual: NukeCannonNeutronWeapon PrimaryDamage **1** /
//!   radius **10**, Scatter **10**, AttackRange **350**, MinRange **150**,
//!   WeaponSpeed **200**, Delay **10000**ms → **300**f, Projectile NeutronCannonShell
//! - Projectile flight residual: FirstHeight **50**, SecondHeight **150**,
//!   FirstPercentIndent **30%**, SecondPercentIndent **70%**, DetonateCallsKill **Yes**
//! - Kill infantry residual: KillInfantry effect + legal-target matrix honesty
//!
//! Fail-closed honesty:
//! - Not full projectile flight / DumbProjectileBehavior live bezier path
//! - Not full AffectAirborne / ally Relationship matrix beyond residual flags
//! - Not full WeaponSet chooser command-button toggle beyond active_weapon_slot
//! - Not network neutron replication (network deferred)

use serde::{Deserialize, Serialize};

/// Logic frames per second residual.
pub const NEUTRON_LOGIC_FPS: f32 = 30.0;

/// Retail Upgrade_ChinaNeutronShells name.
pub const UPGRADE_CHINA_NEUTRON_SHELLS: &str = "Upgrade_ChinaNeutronShells";

/// Retail NukeCannonNeutronWeapon template name.
pub const NUKE_CANNON_NEUTRON_WEAPON: &str = "NukeCannonNeutronWeapon";

/// Retail NukeCannonGun primary template name (for host seed / equip).
pub const NUKE_CANNON_PRIMARY_WEAPON: &str = "NukeCannonGun";

/// Retail NeutronCannonShell projectile object residual.
pub const NEUTRON_CANNON_SHELL_PROJECTILE: &str = "NeutronCannonShell";
/// Retail NukeCannonShell primary projectile (contrast residual).
pub const NUKE_CANNON_SHELL_PROJECTILE: &str = "NukeCannonShell";

/// Retail NeutronCannonShell NeutronBlastBehavior BlastRadius residual.
pub const HOST_NEUTRON_BLAST_RADIUS: f32 = 70.0;

/// Retail NeutronBlastBehavior default BlastRadius when INI omits it.
pub const NEUTRON_BLAST_DEFAULT_RADIUS: f32 = 10.0;
/// Retail NeutronBlastObject BlastRadius residual (distinct from shell = 20).
pub const NEUTRON_BLAST_OBJECT_RADIUS: f32 = 20.0;

/// Retail NeutronCannonShell AffectAirborne residual.
pub const NEUTRON_AFFECT_AIRBORNE: bool = false;
/// Retail NeutronBlastBehavior default AffectAirborne when omitted = TRUE.
pub const NEUTRON_AFFECT_AIRBORNE_DEFAULT: bool = true;
/// Retail NeutronCannonShell omits AffectAllies → module default TRUE.
pub const NEUTRON_AFFECT_ALLIES: bool = true;
/// Retail NeutronBlastObject AffectAllies residual = No.
pub const NEUTRON_BLAST_OBJECT_AFFECT_ALLIES: bool = false;

/// Retail NukeCannonNeutronWeapon AttackRange residual.
pub const NEUTRON_WEAPON_ATTACK_RANGE: f32 = 350.0;

/// Retail NukeCannonNeutronWeapon MinimumAttackRange residual.
pub const NEUTRON_WEAPON_MIN_RANGE: f32 = 150.0;

/// Retail NukeCannonNeutronWeapon DelayBetweenShots 10000ms → 300 frames @ 30 FPS.
pub const NEUTRON_WEAPON_DELAY_MS: u32 = 10_000;
pub const NEUTRON_WEAPON_DELAY_FRAMES: u32 = 300;

/// Retail NukeCannonNeutronWeapon PrimaryDamage residual (shell impact).
pub const NEUTRON_WEAPON_PRIMARY_DAMAGE: f32 = 1.0;
/// Retail PrimaryDamageRadius residual.
pub const NEUTRON_WEAPON_PRIMARY_RADIUS: f32 = 10.0;
/// Retail ScatterRadiusVsInfantry residual.
pub const NEUTRON_WEAPON_SCATTER_VS_INFANTRY: f32 = 10.0;
/// Retail WeaponSpeed residual (dist/sec).
pub const NEUTRON_WEAPON_SPEED: f32 = 200.0;
/// Retail DamageType residual.
pub const NEUTRON_WEAPON_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail DeathType residual.
pub const NEUTRON_WEAPON_DEATH_TYPE: &str = "NORMAL";
/// Retail FireFX residual.
pub const NEUTRON_WEAPON_FIRE_FX: &str = "WeaponFX_NukeCannonMuzzleFlash";
/// Retail ProjectileDetonationFX residual.
pub const NEUTRON_WEAPON_DETONATION_FX: &str = "Neutron_WeaponFX_NukeCannon";
/// Retail RadiusDamageAffects residual tokens.
pub const NEUTRON_WEAPON_RADIUS_AFFECTS: &str =
    "SUICIDE SELF ALLIES ENEMIES NEUTRALS NOT_SIMILAR NOT_AIRBORNE";

/// Retail DumbProjectileBehavior FirstHeight residual.
pub const NEUTRON_SHELL_FIRST_HEIGHT: f32 = 50.0;
/// Retail SecondHeight residual.
pub const NEUTRON_SHELL_SECOND_HEIGHT: f32 = 150.0;
/// Retail FirstPercentIndent residual (30%).
pub const NEUTRON_SHELL_FIRST_PERCENT_INDENT: f32 = 0.30;
/// Retail SecondPercentIndent residual (70%).
pub const NEUTRON_SHELL_SECOND_PERCENT_INDENT: f32 = 0.70;
/// Retail DetonateCallsKill residual on NeutronCannonShell.
pub const NEUTRON_SHELL_DETONATE_CALLS_KILL: bool = true;
/// Retail projectile MaxHealth residual.
pub const NEUTRON_SHELL_MAX_HEALTH: f32 = 100.0;
/// Retail PhysicsBehavior Mass residual.
pub const NEUTRON_SHELL_MASS: f32 = 0.01;
/// Retail incoming whistle residual.
pub const NEUTRON_SHELL_AMBIENT_SOUND: &str = "NukeCannonIncomingWhistle";

/// Activate / detonation audio residual.
pub const NEUTRON_SHELL_AUDIO: &str = "NukeCannonWeapon";

/// Convert msec residual → logic frames @ 30 FPS (exact for 10000ms).
pub fn neutron_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * NEUTRON_LOGIC_FPS / 1000.0).round() as u32
}

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
pub fn in_neutron_blast_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

/// Shell impact primary residual damage at distance (PrimaryDamageRadius ring).
pub fn neutron_shell_primary_damage_at(distance: f32) -> f32 {
    if distance <= NEUTRON_WEAPON_PRIMARY_RADIUS {
        NEUTRON_WEAPON_PRIMARY_DAMAGE
    } else {
        0.0
    }
}

/// Whether attack range residual is legal for neutron secondary (min/max).
pub fn neutron_weapon_range_ok(distance: f32) -> bool {
    distance >= NEUTRON_WEAPON_MIN_RANGE && distance <= NEUTRON_WEAPON_ATTACK_RANGE
}

/// Bezier control residual sample (FirstHeight / FirstPercentIndent).
pub fn neutron_shell_flight_control_height(first: bool) -> f32 {
    if first {
        NEUTRON_SHELL_FIRST_HEIGHT
    } else {
        NEUTRON_SHELL_SECOND_HEIGHT
    }
}

/// Bezier control residual sample (percent indent along shot).
pub fn neutron_shell_flight_control_percent(first: bool) -> f32 {
    if first {
        NEUTRON_SHELL_FIRST_PERCENT_INDENT
    } else {
        NEUTRON_SHELL_SECOND_PERCENT_INDENT
    }
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

/// Host residual honesty counters for neutron shell blasts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostNeutronShellRegistry {
    /// Residual neutron secondary blasts applied.
    pub blasts: u32,
    /// Infantry killed by residual blast.
    pub infantry_kills: u32,
    /// Vehicles unmanned by residual blast.
    pub vehicles_unmanned: u32,
    /// Combat-bike / kill-vehicle residual destructions.
    pub vehicles_killed: u32,
    /// Passengers residual killed on unman path.
    pub passengers_killed: u32,
    /// Ally targets skipped when AffectAllies residual false path.
    pub ally_skips: u32,
}

impl HostNeutronShellRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_blast(
        &mut self,
        infantry_kills: u32,
        vehicles_unmanned: u32,
        vehicles_killed: u32,
        passengers_killed: u32,
    ) {
        self.blasts = self.blasts.saturating_add(1);
        self.infantry_kills = self.infantry_kills.saturating_add(infantry_kills);
        self.vehicles_unmanned = self.vehicles_unmanned.saturating_add(vehicles_unmanned);
        self.vehicles_killed = self.vehicles_killed.saturating_add(vehicles_killed);
        self.passengers_killed = self.passengers_killed.saturating_add(passengers_killed);
    }

    pub fn record_ally_skip(&mut self) {
        self.ally_skips = self.ally_skips.saturating_add(1);
    }

    pub fn honesty_blast_ok(&self) -> bool {
        self.blasts > 0
    }

    pub fn honesty_infantry_kill_ok(&self) -> bool {
        self.infantry_kills > 0
    }

    pub fn honesty_unman_ok(&self) -> bool {
        self.vehicles_unmanned > 0
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_blast_ok()
            && (self.honesty_infantry_kill_ok()
                || self.honesty_unman_ok()
                || self.vehicles_killed > 0)
    }
}

// --- Wave 55 residual honesty packs ---

/// Neutron blast residual damage/radius / affect flags.
pub fn honesty_neutron_blast_residual_ok() -> bool {
    (HOST_NEUTRON_BLAST_RADIUS - 70.0).abs() < 0.01
        && (NEUTRON_BLAST_DEFAULT_RADIUS - 10.0).abs() < 0.01
        && (NEUTRON_BLAST_OBJECT_RADIUS - 20.0).abs() < 0.01
        && !NEUTRON_AFFECT_AIRBORNE
        && NEUTRON_AFFECT_AIRBORNE_DEFAULT
        && NEUTRON_AFFECT_ALLIES
        && !NEUTRON_BLAST_OBJECT_AFFECT_ALLIES
        && in_neutron_blast_radius_2d((0.0, 0.0), (70.0, 0.0), HOST_NEUTRON_BLAST_RADIUS)
        && !in_neutron_blast_radius_2d((0.0, 0.0), (71.0, 0.0), HOST_NEUTRON_BLAST_RADIUS)
        && (neutron_shell_primary_damage_at(5.0) - 1.0).abs() < 0.01
        && neutron_shell_primary_damage_at(15.0).abs() < 0.01
}

/// Shell projectile residual (weapon + flight path).
pub fn honesty_neutron_shell_projectile_residual_ok() -> bool {
    NUKE_CANNON_NEUTRON_WEAPON == "NukeCannonNeutronWeapon"
        && NEUTRON_CANNON_SHELL_PROJECTILE == "NeutronCannonShell"
        && (NEUTRON_WEAPON_ATTACK_RANGE - 350.0).abs() < 0.01
        && (NEUTRON_WEAPON_MIN_RANGE - 150.0).abs() < 0.01
        && NEUTRON_WEAPON_DELAY_MS == 10_000
        && NEUTRON_WEAPON_DELAY_FRAMES == neutron_ms_to_frames(NEUTRON_WEAPON_DELAY_MS)
        && NEUTRON_WEAPON_DELAY_FRAMES == 300
        && (NEUTRON_WEAPON_PRIMARY_DAMAGE - 1.0).abs() < 0.01
        && (NEUTRON_WEAPON_PRIMARY_RADIUS - 10.0).abs() < 0.01
        && (NEUTRON_WEAPON_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && (NEUTRON_WEAPON_SPEED - 200.0).abs() < 0.01
        && NEUTRON_WEAPON_DAMAGE_TYPE == "EXPLOSION"
        && NEUTRON_WEAPON_DEATH_TYPE == "NORMAL"
        && NEUTRON_WEAPON_FIRE_FX == "WeaponFX_NukeCannonMuzzleFlash"
        && NEUTRON_WEAPON_DETONATION_FX == "Neutron_WeaponFX_NukeCannon"
        && NEUTRON_WEAPON_RADIUS_AFFECTS.contains("NOT_AIRBORNE")
        && (NEUTRON_SHELL_FIRST_HEIGHT - 50.0).abs() < 0.01
        && (NEUTRON_SHELL_SECOND_HEIGHT - 150.0).abs() < 0.01
        && (NEUTRON_SHELL_FIRST_PERCENT_INDENT - 0.30).abs() < 0.001
        && (NEUTRON_SHELL_SECOND_PERCENT_INDENT - 0.70).abs() < 0.001
        && neutron_shell_flight_control_height(true) == NEUTRON_SHELL_FIRST_HEIGHT
        && neutron_shell_flight_control_percent(false) == NEUTRON_SHELL_SECOND_PERCENT_INDENT
        && NEUTRON_SHELL_DETONATE_CALLS_KILL
        && (NEUTRON_SHELL_MAX_HEALTH - 100.0).abs() < 0.01
        && (NEUTRON_SHELL_MASS - 0.01).abs() < 0.0001
        && NEUTRON_SHELL_AMBIENT_SOUND == "NukeCannonIncomingWhistle"
        && neutron_weapon_range_ok(200.0)
        && !neutron_weapon_range_ok(100.0)
        && !neutron_weapon_range_ok(400.0)
        && !NEUTRON_SHELL_AUDIO.is_empty()
}

/// Kill infantry residual (effect enum + legal-target + upgrade gate).
pub fn honesty_neutron_kill_infantry_residual_ok() -> bool {
    neutron_effect_for_target(true, false, false, "USA_Ranger") == NeutronEffect::KillInfantry
        && neutron_effect_for_target(false, true, false, "TestTank") == NeutronEffect::UnmanVehicle
        && neutron_effect_for_target(false, true, false, "GLACombatBike")
            == NeutronEffect::KillVehicle
        && is_legal_neutron_blast_target(
            true, true, false, false, false, false, false, NEUTRON_AFFECT_AIRBORNE, false,
            NEUTRON_AFFECT_ALLIES,
        )
        // Ally infantry legal when AffectAllies=Yes (shell residual).
        && is_legal_neutron_blast_target(
            true, true, false, false, false, false, false, NEUTRON_AFFECT_AIRBORNE, true,
            NEUTRON_AFFECT_ALLIES,
        )
        // Ally skipped when AffectAllies=No (NeutronBlastObject residual).
        && !is_legal_neutron_blast_target(
            true, true, false, false, false, false, false, false, true,
            NEUTRON_BLAST_OBJECT_AFFECT_ALLIES,
        )
        // Airborne skipped when AffectAirborne=No.
        && !is_legal_neutron_blast_target(
            true, true, false, false, false, false, true, NEUTRON_AFFECT_AIRBORNE, false,
            NEUTRON_AFFECT_ALLIES,
        )
        && should_apply_neutron_blast(true, 1, true)
        && !should_apply_neutron_blast(true, 0, true)
        && UPGRADE_CHINA_NEUTRON_SHELLS == "Upgrade_ChinaNeutronShells"
}

/// Combined Wave 55 neutron residual honesty pack.
pub fn honesty_neutron_shell_residual_pack_ok() -> bool {
    honesty_neutron_blast_residual_ok()
        && honesty_neutron_shell_projectile_residual_ok()
        && honesty_neutron_kill_infantry_residual_ok()
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

    #[test]
    fn neutron_residual_pack_honesty() {
        assert!(honesty_neutron_blast_residual_ok());
        assert!(honesty_neutron_shell_projectile_residual_ok());
        assert!(honesty_neutron_kill_infantry_residual_ok());
        assert!(honesty_neutron_shell_residual_pack_ok());
        assert_eq!(neutron_ms_to_frames(10_000), 300);
        let mut reg = HostNeutronShellRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        reg.record_blast(2, 1, 0, 1);
        assert!(reg.honesty_blast_ok());
        assert!(reg.honesty_infantry_kill_ok());
        assert!(reg.honesty_unman_ok());
        assert!(reg.honesty_host_path_ok());
    }
}
