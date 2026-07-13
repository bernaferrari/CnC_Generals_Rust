//! Host America Bunker Buster residual (Stealth Fighter + Upgrade_AmericaBunkerBusters).
//!
//! Residual slice (playability):
//! - `Upgrade_AmericaBunkerBusters` research tags residual Stealth Fighters
//!   (C++ BunkerBusterBehavior::UpgradeRequired on StealthJetMissile).
//! - Combat impact on a structure with residual bunker-buster capability:
//!   - Kills all garrisoned occupants (C++ contain killAllContained /
//!     harmAndForceExitAllContained residual).
//!   - Applies amplified structure damage vs bunkers ("damages bunkers more").
//! - Optional KILL_GARRISONED residual (MicrowaveTankBuildingClearer style):
//!   kill `floor(damage)` occupants without requiring the upgrade.
//!
//! Fail-closed honesty:
//! - Not full BunkerBusterBehavior crash-through FX / seismic sim / shockwave
//!   temp weapon (BunkerBusterShockwaveWeaponSmall) path
//! - Not full projectile StealthJetMissile AI / KillSelfDelay crash path
//! - Not full GarrisonContain isBustable / TunnelContain crash-guard matrix
//! - Not network bunker-buster replication (network deferred)

use serde::{Deserialize, Serialize};

/// Retail upgrade that enables bunker-buster residual on Stealth Fighter missiles.
pub const UPGRADE_AMERICA_BUNKER_BUSTERS: &str = "Upgrade_AmericaBunkerBusters";

/// Retail StealthJetMissileWeapon template name (host seed residual).
pub const STEALTH_JET_MISSILE_WEAPON: &str = "StealthJetMissileWeapon";

/// Retail MicrowaveTankBuildingClearer — DamageType = KILL_GARRISONED.
pub const MICROWAVE_BUILDING_CLEARER_WEAPON: &str = "MicrowaveTankBuildingClearer";

/// Residual structure damage multiplier vs bunkers when bunker-buster residual hits.
/// Fail-closed: not full armor / STEALTHJET_MISSILES damage-FX matrix.
pub const BUNKER_BUSTER_STRUCTURE_DAMAGE_MULT: f32 = 1.5;

/// Residual occupant kill damage (matches BunkerBusterAntiTunnel PrimaryDamage 400 residual).
pub const BUNKER_BUSTER_OCCUPANT_DAMAGE: f32 = 400.0;

/// Residual shockwave-style structure bonus when bunker is occupied (host playability).
/// Fail-closed: not full BunkerBusterShockwaveWeaponSmall PrimaryDamage 10 radius 50.
pub const BUNKER_BUSTER_OCCUPIED_BONUS_DAMAGE: f32 = 10.0;

/// Activate / impact audio residual.
pub const BUNKER_BUSTER_AUDIO: &str = "StealthJetMissileWeapon";

/// Whether template is a residual Stealth Fighter / bunker-buster carrier.
///
/// Fail-closed: name residual (not full WeaponSet / JetAIUpdate matrix).
/// Excludes projectile shells (`StealthJetMissile` alone as projectile object).
pub fn is_bunker_buster_carrier(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Projectile / missile objects are not the plane.
    if n == "stealthjetmissile"
        || n.ends_with("jetmissile")
        || n.contains("projectile")
        || n.contains("shell")
    {
        // Keep plane names that include "missile" only if also fighter.
        if !(n.contains("fighter") || n.contains("jetstealth")) {
            return false;
        }
    }
    if n == "teststealthfighter" || n == "testbunkerbuster" {
        return true;
    }
    // Retail BunkerBusterBehavior lives on StealthJetMissile (Stealth Fighter).
    // Fail-closed: Aurora bombers are not bunker-buster carriers.
    n.contains("stealthfighter") || n.contains("jetstealth") || n.contains("stealthjet")
}

/// Whether residual target structure name is a bunker / garrison building.
pub fn is_bunker_structure_name(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("bunker")
        || n.contains("garrison")
        || n == "testbunker"
        || n.contains("tunnelemp")
        || n.contains("tunnelnetwork")
}

/// Whether residual attacker is a KILL_GARRISONED clearer (Microwave Tank residual).
pub fn is_kill_garrisoned_clearer(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("microwave") || n == "testmicrowave" || n.contains("buildingclearer")
}

/// Slot-aware residual: bunker-buster full bust (kill occupants + bunker mult damage).
///
/// C++ BunkerBusterBehavior requires Upgrade_AmericaBunkerBusters on controlling player.
pub fn should_apply_bunker_buster(
    has_upgrade: bool,
    is_carrier: bool,
    target_is_structure: bool,
) -> bool {
    has_upgrade && is_carrier && target_is_structure
}

/// KILL_GARRISONED residual: microwave-style occupant kill without upgrade.
pub fn should_apply_kill_garrisoned(is_clearer: bool, target_is_structure: bool) -> bool {
    is_clearer && target_is_structure
}

/// Structure HP damage residual for bunker buster hit.
///
/// - Base weapon damage always applies.
/// - Bunkers take residual mult (damages bunkers more).
/// - Occupied bunkers get residual occupied bonus (shockwave residual).
pub fn bunker_buster_structure_damage(
    base_damage: f32,
    is_bunker: bool,
    had_occupants: bool,
) -> f32 {
    let mut dmg = base_damage.max(0.0);
    if is_bunker {
        dmg *= BUNKER_BUSTER_STRUCTURE_DAMAGE_MULT;
    }
    if had_occupants {
        dmg += BUNKER_BUSTER_OCCUPIED_BONUS_DAMAGE;
    }
    dmg
}

/// How many occupants KILL_GARRISONED residual kills (C++ amount.floor()).
pub fn kill_garrisoned_count(damage_amount: f32, contained_count: usize) -> usize {
    if contained_count == 0 {
        return 0;
    }
    let kills = damage_amount.floor().max(0.0) as usize;
    kills.min(contained_count)
}

/// Host residual honesty counters for bunker buster / kill-garrisoned.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBunkerBusterRegistry {
    /// Successful bunker-buster residual impacts (upgrade path).
    pub blasts: u32,
    /// Occupants killed by bunker-buster or KILL_GARRISONED residual.
    pub occupants_killed: u32,
    /// Times residual applied amplified bunker structure damage.
    pub bunker_damage_hits: u32,
    /// Total residual structure HP damage applied by bunker-buster path.
    pub structure_damage_dealt: f32,
    /// KILL_GARRISONED clearer residual applications (microwave style).
    pub kill_garrisoned_hits: u32,
}

impl HostBunkerBusterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn record_bunker_buster_blast(
        &mut self,
        occupants_killed: u32,
        structure_damage: f32,
        bunker_amplified: bool,
    ) {
        self.blasts = self.blasts.saturating_add(1);
        self.occupants_killed = self.occupants_killed.saturating_add(occupants_killed);
        self.structure_damage_dealt += structure_damage.max(0.0);
        if bunker_amplified {
            self.bunker_damage_hits = self.bunker_damage_hits.saturating_add(1);
        }
    }

    pub fn record_kill_garrisoned(&mut self, occupants_killed: u32) {
        self.kill_garrisoned_hits = self.kill_garrisoned_hits.saturating_add(1);
        self.occupants_killed = self.occupants_killed.saturating_add(occupants_killed);
    }

    /// Residual honesty: at least one bunker-buster blast applied.
    pub fn honesty_blast_ok(&self) -> bool {
        self.blasts > 0
    }

    /// Residual honesty: at least one garrison occupant killed.
    pub fn honesty_garrison_kill_ok(&self) -> bool {
        self.occupants_killed > 0
    }

    /// Residual honesty: amplified bunker structure damage applied.
    pub fn honesty_bunker_damage_ok(&self) -> bool {
        self.bunker_damage_hits > 0 && self.structure_damage_dealt > 0.0
    }

    /// Combined host path: blast + (garrison kill or bunker damage).
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_blast_ok()
            && (self.honesty_garrison_kill_ok() || self.honesty_bunker_damage_ok())
    }

    /// KILL_GARRISONED residual honesty.
    pub fn honesty_kill_garrisoned_ok(&self) -> bool {
        self.kill_garrisoned_hits > 0 && self.occupants_killed > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carrier_name_matrix() {
        assert!(is_bunker_buster_carrier("AmericaJetStealthFighter"));
        assert!(is_bunker_buster_carrier("USA_StealthFighter"));
        assert!(is_bunker_buster_carrier("SupW_AmericaJetStealthFighter"));
        assert!(is_bunker_buster_carrier("TestStealthFighter"));
        assert!(!is_bunker_buster_carrier("AmericaJetAurora"));
        assert!(!is_bunker_buster_carrier("StealthJetMissile"));
        assert!(!is_bunker_buster_carrier("USA_Ranger"));
        assert!(!is_bunker_buster_carrier("ChinaVehicleNukeCannon"));
    }

    #[test]
    fn bunker_structure_name_matrix() {
        assert!(is_bunker_structure_name("TestBunker"));
        assert!(is_bunker_structure_name("AmericaBunker"));
        assert!(is_bunker_structure_name("CivilianBunker"));
        assert!(!is_bunker_structure_name("TestBarracks"));
        assert!(!is_bunker_structure_name("USA_CommandCenter"));
    }

    #[test]
    fn should_apply_gates() {
        assert!(should_apply_bunker_buster(true, true, true));
        assert!(!should_apply_bunker_buster(false, true, true));
        assert!(!should_apply_bunker_buster(true, false, true));
        assert!(!should_apply_bunker_buster(true, true, false));
        assert!(should_apply_kill_garrisoned(true, true));
        assert!(!should_apply_kill_garrisoned(true, false));
    }

    #[test]
    fn structure_damage_amplifies_bunkers() {
        let base = 100.0;
        let normal = bunker_buster_structure_damage(base, false, false);
        assert!((normal - 100.0).abs() < 0.01);
        let bunker = bunker_buster_structure_damage(base, true, false);
        assert!(
            (bunker - 150.0).abs() < 0.01,
            "bunker mult residual 1.5x, got {bunker}"
        );
        let occupied = bunker_buster_structure_damage(base, true, true);
        assert!(
            (occupied - 160.0).abs() < 0.01,
            "occupied bunker +10 residual, got {occupied}"
        );
    }

    #[test]
    fn kill_garrisoned_count_matrix() {
        assert_eq!(kill_garrisoned_count(1.0, 5), 1);
        assert_eq!(kill_garrisoned_count(3.5, 5), 3);
        assert_eq!(kill_garrisoned_count(10.0, 2), 2);
        assert_eq!(kill_garrisoned_count(0.0, 5), 0);
        assert_eq!(kill_garrisoned_count(5.0, 0), 0);
    }

    #[test]
    fn honesty_tracks_blast_and_kills() {
        let mut reg = HostBunkerBusterRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        reg.record_bunker_buster_blast(2, 160.0, true);
        assert!(reg.honesty_blast_ok());
        assert!(reg.honesty_garrison_kill_ok());
        assert!(reg.honesty_bunker_damage_ok());
        assert!(reg.honesty_host_path_ok());
        reg.record_kill_garrisoned(1);
        assert!(reg.honesty_kill_garrisoned_ok());
        assert_eq!(reg.occupants_killed, 3);
    }
}
