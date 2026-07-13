//! Host Demo General SuicideBomb residual (Demo_Upgrade_SuicideBomb death blast).
//!
//! Residual slice (playability):
//! - `Demo_Upgrade_SuicideBomb` QueueUpgrade → complete tags eligible Demo units /
//!   structures with residual death-weapon readiness.
//! - On death (non-SUICIDED residual path), tagged Demo units/structures apply
//!   `Demo_DestroyedWeapon`: Primary **50**/r**60** + Secondary **10**/r**70**.
//! - FireFX residual honesty: PlusFire detonation audio/particle path name.
//! - Fail-closed: Terrorist SUICIDED residual remains `Demo_SuicideDynamitePack`
//!   (700 primary) via host_terrorist — not switched here.
//!
//! Fail-closed honesty:
//! - Not full FireWeaponWhenDead exclusive RequiresAllTriggers module matrix
//! - Not full SUICIDED → Demo_SuicideDynamitePackPlusFire (500 primary) path for
//!   non-terrorist units (host collapses to Demo_DestroyedWeapon on normal death)
//! - Not full CommandSetUpgrade residual for suicide-bomb command sets
//! - Not network suicide-bomb replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Retail Demo_Upgrade_SuicideBomb.
pub const UPGRADE_DEMO_SUICIDE_BOMB: &str = "Demo_Upgrade_SuicideBomb";
/// Retail death weapon for non-SUICIDED deaths with SuicideBomb upgrade.
pub const DEMO_DESTROYED_WEAPON: &str = "Demo_DestroyedWeapon";
/// Retail SUICIDED death weapon for SuicideBomb-tagged non-terrorist residual.
pub const DEMO_SUICIDE_DYNAMITE_PLUS_FIRE: &str = "Demo_SuicideDynamitePackPlusFire";

/// Demo_DestroyedWeapon PrimaryDamage residual.
pub const DEMO_DESTROYED_PRIMARY_DAMAGE: f32 = 50.0;
/// Demo_DestroyedWeapon PrimaryDamageRadius residual.
pub const DEMO_DESTROYED_PRIMARY_RADIUS: f32 = 60.0;
/// Demo_DestroyedWeapon SecondaryDamage residual.
pub const DEMO_DESTROYED_SECONDARY_DAMAGE: f32 = 10.0;
/// Demo_DestroyedWeapon SecondaryDamageRadius residual.
pub const DEMO_DESTROYED_SECONDARY_RADIUS: f32 = 70.0;

/// Demo_SuicideDynamitePackPlusFire PrimaryDamage residual.
pub const DEMO_PLUS_FIRE_PRIMARY_DAMAGE: f32 = 500.0;
/// Demo_SuicideDynamitePackPlusFire PrimaryDamageRadius residual.
pub const DEMO_PLUS_FIRE_PRIMARY_RADIUS: f32 = 18.0;
/// Demo_SuicideDynamitePackPlusFire SecondaryDamage residual.
pub const DEMO_PLUS_FIRE_SECONDARY_DAMAGE: f32 = 300.0;
/// Demo_SuicideDynamitePackPlusFire SecondaryDamageRadius residual.
pub const DEMO_PLUS_FIRE_SECONDARY_RADIUS: f32 = 50.0;

/// Residual detonation audio (retail FireSound = CarBomberDie).
pub const DEMO_SUICIDE_BOMB_AUDIO: &str = "CarBomberDie";

/// Normalize upgrade / template identity.
pub fn normalize_identity(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Whether upgrade name is Demo_Upgrade_SuicideBomb residual.
pub fn is_demo_suicide_bomb_upgrade(name: &str) -> bool {
    let n = normalize_identity(name);
    n.contains("demosuicidebomb")
        || n.contains("upgradesuicidebomb")
        || n == "suicidebomb"
        || n.contains("demoupgradesuicidebomb")
}

/// Whether template is a Demo General living unit/structure residual.
///
/// Fail-closed: name residual. Excludes weapons, effects, debris, holes, sciences.
pub fn is_demo_suicide_bomb_eligible_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Explicit residual test names.
    if n == "testdemosuicidebomb"
        || n == "testdemostructure"
        || n == "testdemorebel"
        || n == "testdemotunnel"
    {
        return true;
    }
    // Must be Demo general prefix or host test alias.
    let is_demo = n.starts_with("demo_")
        || n.starts_with("demo")
        || n.contains("testdemo");
    if !is_demo {
        return false;
    }
    // Exclude non-living residual tokens.
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("shell")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("deatheffect")
        || n.contains("effect")
        || n.contains("hole")
        || n.contains("dynamite")
        || n.contains("suicide") && !n.contains("infantry") && !n.contains("vehicle") && !n.contains("structure")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("commandset")
    {
        return false;
    }
    // Living Demo combatants / structures (not pure cinematic junk).
    n.contains("infantry")
        || n.contains("vehicle")
        || n.contains("tunnel")
        || n.contains("stinger")
        || n.contains("palace")
        || n.contains("barracks")
        || n.contains("armsdealer")
        || n.contains("supply")
        || n.contains("commandcenter")
        || n.contains("blackmarket")
        || n.contains("scudstorm")
        || n.contains("demotrap")
        || n.contains("rebel")
        || n.contains("rpg")
        || n.contains("hijacker")
        || n.contains("jarmen")
        || n.contains("worker")
        || n.contains("angrymob")
        || n.contains("quad")
        || n.contains("technical")
        || n.contains("scorpion")
        || n.contains("marauder")
        || n.contains("rocketbuggy")
        || n.contains("scud")
        || n.contains("bombtruck")
        || n.contains("battlebus")
        || n.contains("combatcycle")
        || n.contains("radarvan")
        || n.contains("testdemo")
}

/// Whether residual target receives Demo_DestroyedWeapon splash.
pub fn is_legal_demo_destroyed_target(
    alive: bool,
    is_self: bool,
    under_construction: bool,
) -> bool {
    alive && !is_self && !under_construction
}

/// Dual-ring residual damage at distance for Demo_DestroyedWeapon.
pub fn demo_destroyed_damage_at(distance: f32) -> f32 {
    if distance <= DEMO_DESTROYED_PRIMARY_RADIUS {
        DEMO_DESTROYED_PRIMARY_DAMAGE
    } else if distance <= DEMO_DESTROYED_SECONDARY_RADIUS {
        DEMO_DESTROYED_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// Dual-ring residual damage at distance for PlusFire (SUICIDED residual honesty).
pub fn demo_plus_fire_damage_at(distance: f32) -> f32 {
    if distance <= DEMO_PLUS_FIRE_PRIMARY_RADIUS {
        DEMO_PLUS_FIRE_PRIMARY_DAMAGE
    } else if distance <= DEMO_PLUS_FIRE_SECONDARY_RADIUS {
        DEMO_PLUS_FIRE_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// Whether object tags indicate SuicideBomb residual is armed.
pub fn has_demo_suicide_bomb_upgrade(
    applied_upgrades: &std::collections::HashSet<String>,
) -> bool {
    applied_upgrades
        .iter()
        .any(|u| is_demo_suicide_bomb_upgrade(u))
}

/// Host residual honesty registry for Demo SuicideBomb.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostDemoSuicideBombRegistry {
    /// Units tagged by upgrade complete residual.
    pub units_tagged: u32,
    /// Death detonations resolved (Demo_DestroyedWeapon residual).
    pub death_detonations: u32,
    /// Objects hit by residual blast.
    pub blast_hits: u32,
    /// Total residual blast damage dealt.
    pub blast_damage_dealt: f32,
    /// Objects destroyed by residual blast.
    pub objects_destroyed: u32,
    /// Upgrade complete events.
    pub upgrade_completes: u32,
}

impl HostDemoSuicideBombRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_upgrade_complete(&mut self, units_tagged: u32) {
        self.upgrade_completes = self.upgrade_completes.saturating_add(1);
        self.units_tagged = self.units_tagged.saturating_add(units_tagged);
    }

    pub fn record_tag(&mut self) {
        self.units_tagged = self.units_tagged.saturating_add(1);
    }

    pub fn record_death_detonation(&mut self, blast_hits: u32, blast_damage: f32, destroyed: u32) {
        self.death_detonations = self.death_detonations.saturating_add(1);
        self.blast_hits = self.blast_hits.saturating_add(blast_hits);
        if blast_damage > 0.0 {
            self.blast_damage_dealt += blast_damage;
        }
        self.objects_destroyed = self.objects_destroyed.saturating_add(destroyed);
    }

    pub fn honesty_upgrade_ok(&self) -> bool {
        // Upgrade researched (tags may arrive later on spawn residual).
        self.upgrade_completes > 0
    }

    pub fn honesty_death_ok(&self) -> bool {
        self.death_detonations > 0 && self.blast_hits > 0
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        // Full residual path: research + at least one death detonation.
        self.honesty_upgrade_ok() && self.honesty_death_ok()
    }
}

/// One residual blast hit planned for Demo_DestroyedWeapon.
#[derive(Debug, Clone, Copy)]
pub struct HostDemoSuicideBombHit {
    pub target_id: ObjectId,
    pub damage: f32,
}

/// Plan residual Demo_DestroyedWeapon hits around a death position.
pub fn plan_demo_destroyed_hits(
    source_id: ObjectId,
    source_pos: Vec3,
    candidates: &[(ObjectId, Vec3, bool, bool)],
) -> Vec<HostDemoSuicideBombHit> {
    let mut hits = Vec::new();
    let max_r = DEMO_DESTROYED_SECONDARY_RADIUS;
    for &(id, pos, alive, under_construction) in candidates {
        if !is_legal_demo_destroyed_target(alive, id == source_id, under_construction) {
            continue;
        }
        let dx = pos.x - source_pos.x;
        let dz = pos.z - source_pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist > max_r {
            continue;
        }
        let dmg = demo_destroyed_damage_at(dist);
        if dmg > 0.0 {
            hits.push(HostDemoSuicideBombHit {
                target_id: id,
                damage: dmg,
            });
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_and_template_matrix() {
        assert!(is_demo_suicide_bomb_upgrade(UPGRADE_DEMO_SUICIDE_BOMB));
        assert!(is_demo_suicide_bomb_upgrade("Demo_Upgrade_SuicideBomb"));
        assert!(!is_demo_suicide_bomb_upgrade("Upgrade_GLACamouflage"));

        assert!(is_demo_suicide_bomb_eligible_template("Demo_GLAInfantryRebel"));
        assert!(is_demo_suicide_bomb_eligible_template("Demo_GLATunnelNetwork"));
        assert!(is_demo_suicide_bomb_eligible_template("Demo_GLAStingerSite"));
        assert!(is_demo_suicide_bomb_eligible_template("Demo_GLAVehicleScorpion"));
        assert!(is_demo_suicide_bomb_eligible_template("TestDemoRebel"));
        assert!(!is_demo_suicide_bomb_eligible_template("GLAInfantryRebel"));
        assert!(!is_demo_suicide_bomb_eligible_template("Demo_SuicideDynamitePack"));
        assert!(!is_demo_suicide_bomb_eligible_template("Demo_DestroyedWeapon"));
        assert!(!is_demo_suicide_bomb_eligible_template("Demo_Upgrade_SuicideBomb"));
        assert!(!is_demo_suicide_bomb_eligible_template("Demo_GLAHoleTunnelNetwork"));
    }

    #[test]
    fn destroyed_weapon_rings() {
        assert!((demo_destroyed_damage_at(0.0) - 50.0).abs() < 0.01);
        assert!((demo_destroyed_damage_at(60.0) - 50.0).abs() < 0.01);
        assert!((demo_destroyed_damage_at(65.0) - 10.0).abs() < 0.01);
        assert!((demo_destroyed_damage_at(71.0)).abs() < 0.01);
        assert!((demo_plus_fire_damage_at(0.0) - 500.0).abs() < 0.01);
        assert!((demo_plus_fire_damage_at(25.0) - 300.0).abs() < 0.01);
    }

    #[test]
    fn registry_honesty() {
        let mut reg = HostDemoSuicideBombRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        reg.record_upgrade_complete(0);
        assert!(reg.honesty_upgrade_ok());
        reg.record_death_detonation(2, 60.0, 0);
        assert!(reg.honesty_death_ok());
        assert!(reg.honesty_host_path_ok());
    }

    #[test]
    fn plan_hits_skips_self_and_dead() {
        let src = ObjectId(1);
        let alive = ObjectId(2);
        let dead = ObjectId(3);
        let hits = plan_demo_destroyed_hits(
            src,
            Vec3::ZERO,
            &[
                (src, Vec3::ZERO, true, false),
                (alive, Vec3::new(10.0, 0.0, 0.0), true, false),
                (dead, Vec3::new(5.0, 0.0, 0.0), false, false),
            ],
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].target_id, alive);
        assert!((hits[0].damage - 50.0).abs() < 0.01);
    }
}
