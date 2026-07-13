//! Host GLA Tunnel Network residual.
//!
//! Residual slice (playability):
//! - `TunnelContain` shared passenger pool per team (`GameData.ini MaxTunnelCapacity = 10`)
//! - Enter any allied tunnel network structure
//! - Exit / Evacuate at **any** allied tunnel (cross-tunnel residual path)
//! - C++ valid container: all units except aircraft (Kris 2002 / srj aircraft skip)
//! - Structure PRIMARY `TunnelNetworkGun` residual auto-fire (dmg **15** /
//!   range **175** / Delay **250**ms → 8 frames) via base-defense residual path
//!
//! Wave 64 residual pack (retail FactionBuilding.ini / Weapon.ini / GameData.ini):
//! - Body: MaxHealth **1000**, BuildCost **800**, BuildTime **15**s → **450**f,
//!   Vision/Shroud **200**, EnergyProduction **0**, TurretTurnRate **180**
//! - TunnelContain: TimeForFullHeal **5000**ms → **150**f, MaxTunnelCapacity **10**
//! - TunnelNetworkGun: dmg **15** / range **175** / Delay **250**ms → **8**f /
//!   WeaponSpeed **600** / FireSound HumveeWeapon / FireFX WeaponFX_TechnicalGunFire
//! - StealthDetectorUpdate: DetectionRate **500**ms → **15**f, DetectionRange **150**
//! - SpawnBehavior residual: SpawnNumber **2**, GLAInfantryTunnelDefender OneShot
//! - CamoNetting residual: Upgrade_GLACamoNetting, StealthDelay **2500**ms → **75**f,
//!   Forbidden ATTACKING USING_ABILITY TAKING_DAMAGE
//! - RebuildHole residual: GLAHoleTunnelNetwork HoleMaxHealth **500**
//!
//! Fail-closed honesty:
//! - Not full GuardTunnelNetwork AI / AITNGuard nemesis path
//! - Not full TimeForFullHeal matrix / healObjects tick
//! - Not CaveSystem multi-index / last-tunnel cave-in destroy matrix
//! - Not full ExitStart bone / multi-door exit interface
//! - Not full SneakAttack TunnelNetworkGunDUMMY zero-damage matrix (real gun residual)
//! - Not network tunnel-network replication (network deferred)

use super::{ObjectId, Team, Weapon};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const TUNNEL_NETWORK_LOGIC_FPS: f32 = 30.0;

/// C++ `GameData.ini` `MaxTunnelCapacity = 10`.
pub const MAX_TUNNEL_CAPACITY: usize = 10;

/// Residual of TunnelContain `TimeForFullHeal = 5000` ms.
pub const TUNNEL_FULL_HEAL_MS: u32 = 5000;
/// Residual of TunnelContain `TimeForFullHeal = 5000` ms → frames @ 30 FPS.
/// Fail-closed: heal tick not required for enter/exit honesty.
pub const TUNNEL_FULL_HEAL_FRAMES: u32 = 150;

/// Retail TunnelNetworkGun primary weapon template name.
pub const TUNNEL_NETWORK_GUN: &str = "TunnelNetworkGun";
/// Retail TunnelNetworkGun PrimaryDamage.
pub const TUNNEL_NETWORK_GUN_DAMAGE: f32 = 15.0;
/// Retail TunnelNetworkGun AttackRange.
pub const TUNNEL_NETWORK_GUN_RANGE: f32 = 175.0;
/// Retail DelayBetweenShots residual (msec).
pub const TUNNEL_NETWORK_GUN_DELAY_MS: u32 = 250;
/// Retail DelayBetweenShots 250ms → 8 frames @ 30 FPS.
pub const TUNNEL_NETWORK_GUN_DELAY_FRAMES: u32 = 8;
/// Retail WeaponSpeed residual (dist/sec).
pub const TUNNEL_NETWORK_GUN_WEAPON_SPEED: f32 = 600.0;
/// Residual fire audio (retail FireSound = HumveeWeapon).
pub const TUNNEL_NETWORK_GUN_AUDIO: &str = "HumveeWeapon";
/// Retail FireFX residual.
pub const TUNNEL_NETWORK_GUN_FIRE_FX: &str = "WeaponFX_TechnicalGunFire";

/// Retail StructureBody MaxHealth residual.
pub const TUNNEL_NETWORK_MAX_HEALTH: f32 = 1000.0;
/// Retail BuildCost residual.
pub const TUNNEL_NETWORK_BUILD_COST: u32 = 800;
/// Retail BuildTime residual (seconds).
pub const TUNNEL_NETWORK_BUILD_TIME_SEC: f32 = 15.0;
/// BuildTime 15s → 450 frames @ 30 FPS.
pub const TUNNEL_NETWORK_BUILD_TIME_FRAMES: u32 = 450;
/// Retail EnergyProduction residual.
pub const TUNNEL_NETWORK_ENERGY_PRODUCTION: i32 = 0;
/// Retail VisionRange residual.
pub const TUNNEL_NETWORK_VISION_RANGE: f32 = 200.0;
/// Retail ShroudClearingRange residual.
pub const TUNNEL_NETWORK_SHROUD_CLEARING_RANGE: f32 = 200.0;
/// Retail AIUpdateInterface TurretTurnRate residual (deg/sec).
pub const TUNNEL_NETWORK_TURRET_TURN_RATE: f32 = 180.0;

/// Retail StealthDetectorUpdate DetectionRate residual (msec).
pub const TUNNEL_NETWORK_DETECTION_RATE_MS: u32 = 500;
/// DetectionRate 500ms → 15 frames @ 30 FPS.
pub const TUNNEL_NETWORK_DETECTION_RATE_FRAMES: u32 = 15;
/// Retail StealthDetectorUpdate DetectionRange residual.
pub const TUNNEL_NETWORK_DETECTION_RANGE: f32 = 150.0;

/// Retail SpawnBehavior SpawnNumber residual.
pub const TUNNEL_NETWORK_SPAWN_NUMBER: u32 = 2;
/// Retail SpawnTemplateName residual.
pub const TUNNEL_NETWORK_SPAWN_TEMPLATE: &str = "GLAInfantryTunnelDefender";
/// Retail SpawnBehavior OneShot residual.
pub const TUNNEL_NETWORK_SPAWN_ONE_SHOT: bool = true;

/// Retail StealthUpgrade TriggeredBy residual.
pub const TUNNEL_NETWORK_CAMO_NETTING_UPGRADE: &str = "Upgrade_GLACamoNetting";
/// Retail StealthUpdate StealthDelay residual (msec).
pub const TUNNEL_NETWORK_STEALTH_DELAY_MS: u32 = 2500;
/// StealthDelay 2500ms → 75 frames @ 30 FPS.
pub const TUNNEL_NETWORK_STEALTH_DELAY_FRAMES: u32 = 75;
/// Retail StealthForbiddenConditions residual tokens.
pub const TUNNEL_NETWORK_STEALTH_FORBIDDEN: &str = "ATTACKING USING_ABILITY TAKING_DAMAGE";

/// Retail RebuildHoleExposeDie HoleName residual.
pub const TUNNEL_NETWORK_HOLE_NAME: &str = "GLAHoleTunnelNetwork";
/// Retail RebuildHoleExposeDie HoleMaxHealth residual.
pub const TUNNEL_NETWORK_HOLE_MAX_HEALTH: f32 = 500.0;

/// Convert residual milliseconds to logic frames @ 30 FPS.
pub fn tunnel_network_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / TUNNEL_NETWORK_LOGIC_FPS)).round() as u32
}

/// Host residual honesty counters + per-team shared contain state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostTunnelNetworkRegistry {
    /// Successful residual enters into any team tunnel.
    pub enters: u32,
    /// Successful residual exits (same or cross tunnel).
    pub exits: u32,
    /// Exits where exit tunnel != entry tunnel (the key residual path).
    pub cross_exits: u32,
    /// Residual TunnelNetworkGun auto-fire honesty shots.
    pub gun_fires: u32,
    /// Residual units hit by TunnelNetworkGun residual.
    pub gun_units_hit: u32,
    /// Per-team shared passenger lists (C++ Player::TunnelTracker contain list).
    networks: HashMap<Team, TeamTunnelNetwork>,
}

/// Shared contain state for one team's tunnel network.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TeamTunnelNetwork {
    /// Units currently inside the communal tunnel pool.
    pub contained: Vec<ObjectId>,
    /// Unit → tunnel they entered (scripts / cross-exit honesty).
    pub entry_tunnel: HashMap<u32, ObjectId>,
}

impl HostTunnelNetworkRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn network(&self, team: Team) -> Option<&TeamTunnelNetwork> {
        self.networks.get(&team)
    }

    pub fn network_mut(&mut self, team: Team) -> &mut TeamTunnelNetwork {
        self.networks.entry(team).or_default()
    }

    pub fn contain_count(&self, team: Team) -> usize {
        self.networks
            .get(&team)
            .map(|n| n.contained.len())
            .unwrap_or(0)
    }

    pub fn has_capacity(&self, team: Team) -> bool {
        self.contain_count(team) < MAX_TUNNEL_CAPACITY
    }

    pub fn is_in_network(&self, team: Team, unit_id: ObjectId) -> bool {
        self.networks
            .get(&team)
            .map(|n| n.contained.contains(&unit_id))
            .unwrap_or(false)
    }

    /// Find which team (if any) currently holds this unit in a tunnel pool.
    pub fn team_holding_unit(&self, unit_id: ObjectId) -> Option<Team> {
        for (team, net) in &self.networks {
            if net.contained.contains(&unit_id) {
                return Some(*team);
            }
        }
        None
    }

    pub fn entry_tunnel_of(&self, unit_id: ObjectId) -> Option<ObjectId> {
        for net in self.networks.values() {
            if let Some(&tid) = net.entry_tunnel.get(&unit_id.0) {
                return Some(tid);
            }
        }
        None
    }

    /// List all unit IDs currently in the team's shared tunnel pool.
    pub fn contained_for_team(&self, team: Team) -> Vec<ObjectId> {
        self.networks
            .get(&team)
            .map(|n| n.contained.clone())
            .unwrap_or_default()
    }

    /// Record enter into shared pool at `entry_tunnel`.
    /// Returns false if capacity full. If already contained, keeps original
    /// entry tunnel (cross-exit honesty: enter A then leave via B).
    pub fn record_enter(&mut self, team: Team, unit_id: ObjectId, entry_tunnel: ObjectId) -> bool {
        if self.is_in_network(team, unit_id) {
            // Already in shared pool (transfer residual between entrances).
            // Preserve original entry tunnel for cross-exit honesty.
            let _ = entry_tunnel;
            return true;
        }
        if !self.has_capacity(team) {
            return false;
        }
        let net = self.network_mut(team);
        net.contained.push(unit_id);
        net.entry_tunnel.insert(unit_id.0, entry_tunnel);
        self.enters = self.enters.saturating_add(1);
        true
    }

    /// Remove unit from shared pool. Returns entry tunnel if it was contained.
    pub fn record_exit(
        &mut self,
        team: Team,
        unit_id: ObjectId,
        exit_tunnel: ObjectId,
    ) -> Option<ObjectId> {
        let net = self.networks.get_mut(&team)?;
        let pos = net.contained.iter().position(|&id| id == unit_id)?;
        net.contained.remove(pos);
        let entry = net.entry_tunnel.remove(&unit_id.0);
        self.exits = self.exits.saturating_add(1);
        if let Some(entry_id) = entry {
            if entry_id != exit_tunnel {
                self.cross_exits = self.cross_exits.saturating_add(1);
            }
        }
        entry
    }

    /// Residual honesty: enter then exit exercised.
    pub fn honesty_enter_exit_ok(&self) -> bool {
        self.enters > 0 && self.exits > 0
    }

    /// Residual honesty: at least one cross-tunnel exit (enter A, exit B).
    pub fn honesty_cross_exit_ok(&self) -> bool {
        self.cross_exits > 0
    }

    /// Record residual TunnelNetworkGun auto-fire shot.
    pub fn record_gun_fire(&mut self, hit: bool) {
        self.gun_fires = self.gun_fires.saturating_add(1);
        if hit {
            self.gun_units_hit = self.gun_units_hit.saturating_add(1);
        }
    }

    /// Residual honesty: TunnelNetworkGun auto-fire residual exercised.
    pub fn honesty_gun_fire_ok(&self) -> bool {
        self.gun_fires > 0 && self.gun_units_hit > 0
    }

    /// Combined residual path honesty.
    pub fn honesty_any_ok(&self) -> bool {
        self.honesty_enter_exit_ok() || self.honesty_cross_exit_ok() || self.honesty_gun_fire_ok()
    }
}

/// Build residual TunnelNetworkGun weapon.
pub fn tunnel_network_gun_weapon() -> Weapon {
    Weapon {
        damage: TUNNEL_NETWORK_GUN_DAMAGE,
        range: TUNNEL_NETWORK_GUN_RANGE,
        min_range: 0.0,
        reload_time: TUNNEL_NETWORK_GUN_DELAY_FRAMES as f32 / TUNNEL_NETWORK_LOGIC_FPS,
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: TUNNEL_NETWORK_GUN_WEAPON_SPEED,
        pre_attack_delay: 0.0,
    }
}

// --- Wave 64 residual honesty packs ---

/// Wave 64 residual honesty: TunnelNetworkGun residual.
pub fn honesty_tunnel_network_gun_residual_ok() -> bool {
    TUNNEL_NETWORK_GUN == "TunnelNetworkGun"
        && (TUNNEL_NETWORK_GUN_DAMAGE - 15.0).abs() < 0.01
        && (TUNNEL_NETWORK_GUN_RANGE - 175.0).abs() < 0.01
        && TUNNEL_NETWORK_GUN_DELAY_MS == 250
        && TUNNEL_NETWORK_GUN_DELAY_FRAMES
            == tunnel_network_ms_to_frames(TUNNEL_NETWORK_GUN_DELAY_MS)
        && (TUNNEL_NETWORK_GUN_WEAPON_SPEED - 600.0).abs() < 0.01
        && TUNNEL_NETWORK_GUN_AUDIO == "HumveeWeapon"
        && TUNNEL_NETWORK_GUN_FIRE_FX == "WeaponFX_TechnicalGunFire"
}

/// Wave 64 residual honesty: TunnelContain + capacity residual.
pub fn honesty_tunnel_network_contain_residual_ok() -> bool {
    MAX_TUNNEL_CAPACITY == 10
        && TUNNEL_FULL_HEAL_MS == 5000
        && TUNNEL_FULL_HEAL_FRAMES == tunnel_network_ms_to_frames(TUNNEL_FULL_HEAL_MS)
}

/// Wave 64 residual honesty: body / build residual.
pub fn honesty_tunnel_network_body_residual_ok() -> bool {
    (TUNNEL_NETWORK_MAX_HEALTH - 1000.0).abs() < 0.01
        && TUNNEL_NETWORK_BUILD_COST == 800
        && (TUNNEL_NETWORK_BUILD_TIME_SEC - 15.0).abs() < 0.01
        && TUNNEL_NETWORK_BUILD_TIME_FRAMES
            == (TUNNEL_NETWORK_BUILD_TIME_SEC * TUNNEL_NETWORK_LOGIC_FPS).round() as u32
        && TUNNEL_NETWORK_ENERGY_PRODUCTION == 0
        && (TUNNEL_NETWORK_VISION_RANGE - 200.0).abs() < 0.01
        && (TUNNEL_NETWORK_SHROUD_CLEARING_RANGE - 200.0).abs() < 0.01
        && (TUNNEL_NETWORK_TURRET_TURN_RATE - 180.0).abs() < 0.01
}

/// Wave 64 residual honesty: detector + spawn residual.
pub fn honesty_tunnel_network_detector_spawn_residual_ok() -> bool {
    TUNNEL_NETWORK_DETECTION_RATE_MS == 500
        && TUNNEL_NETWORK_DETECTION_RATE_FRAMES
            == tunnel_network_ms_to_frames(TUNNEL_NETWORK_DETECTION_RATE_MS)
        && (TUNNEL_NETWORK_DETECTION_RANGE - 150.0).abs() < 0.01
        && TUNNEL_NETWORK_SPAWN_NUMBER == 2
        && TUNNEL_NETWORK_SPAWN_TEMPLATE == "GLAInfantryTunnelDefender"
        && TUNNEL_NETWORK_SPAWN_ONE_SHOT
}

/// Wave 64 residual honesty: CamoNetting + rebuild hole residual.
pub fn honesty_tunnel_network_camo_hole_residual_ok() -> bool {
    TUNNEL_NETWORK_CAMO_NETTING_UPGRADE == "Upgrade_GLACamoNetting"
        && TUNNEL_NETWORK_STEALTH_DELAY_MS == 2500
        && TUNNEL_NETWORK_STEALTH_DELAY_FRAMES
            == tunnel_network_ms_to_frames(TUNNEL_NETWORK_STEALTH_DELAY_MS)
        && TUNNEL_NETWORK_STEALTH_FORBIDDEN == "ATTACKING USING_ABILITY TAKING_DAMAGE"
        && TUNNEL_NETWORK_HOLE_NAME == "GLAHoleTunnelNetwork"
        && (TUNNEL_NETWORK_HOLE_MAX_HEALTH - 500.0).abs() < 0.01
}

/// Combined Wave 64 Tunnel Network residual honesty pack.
pub fn honesty_tunnel_network_residual_pack_ok() -> bool {
    honesty_tunnel_network_gun_residual_ok()
        && honesty_tunnel_network_contain_residual_ok()
        && honesty_tunnel_network_body_residual_ok()
        && honesty_tunnel_network_detector_spawn_residual_ok()
        && honesty_tunnel_network_camo_hole_residual_ok()
}

/// True when template is a GLA (or general) Tunnel Network residual structure.
/// Matches `GLATunnelNetwork`, `GLASneakAttackTunnelNetwork`, `Demo_*`, `Chem_*`,
/// `TestTunnelNetwork`. Excludes hole rubble and sneak-attack Start lifetime objects.
pub fn is_tunnel_network_template(template_name: &str) -> bool {
    let lower = template_name.to_ascii_lowercase();
    if !lower.contains("tunnelnetwork") && !lower.contains("tunnel_network") {
        return false;
    }
    // Hole / Start / NoSpawn residual skip (not usable TunnelContain entrances).
    if lower.contains("hole") || lower.contains("start") || lower.contains("nospawn") {
        return false;
    }
    true
}

/// C++ TunnelTracker::isValidContainerFor residual: reject aircraft only.
pub fn unit_can_use_tunnel(is_aircraft: bool, is_alive: bool, under_construction: bool) -> bool {
    is_alive && !under_construction && !is_aircraft
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_detection_matches_gla_and_variants() {
        assert!(is_tunnel_network_template("GLATunnelNetwork"));
        assert!(is_tunnel_network_template("GLASneakAttackTunnelNetwork"));
        assert!(is_tunnel_network_template("Demo_GLATunnelNetwork"));
        assert!(is_tunnel_network_template("Chem_GLATunnelNetwork"));
        assert!(is_tunnel_network_template("TestTunnelNetwork"));
        assert!(!is_tunnel_network_template("GLAHoleTunnelNetwork"));
        assert!(!is_tunnel_network_template("GLASneakAttackTunnelNetworkStart"));
        assert!(!is_tunnel_network_template("GLATunnelNetworkNoSpawn"));
        assert!(!is_tunnel_network_template("GLA_Barracks"));
        assert!(!is_tunnel_network_template("TestBunker"));
    }

    #[test]
    fn tunnel_network_gun_stats() {
        let w = tunnel_network_gun_weapon();
        assert!((w.damage - TUNNEL_NETWORK_GUN_DAMAGE).abs() < 0.01);
        assert!((w.range - TUNNEL_NETWORK_GUN_RANGE).abs() < 0.01);
        assert!((w.reload_time - (8.0 / 30.0)).abs() < 0.001);
        assert!(w.can_target_ground);
        assert!(!w.can_target_air);
        let mut reg = HostTunnelNetworkRegistry::new();
        assert!(!reg.honesty_gun_fire_ok());
        reg.record_gun_fire(true);
        assert!(reg.honesty_gun_fire_ok());
    }

    #[test]
    fn capacity_and_enter_exit_shared_pool() {
        let mut reg = HostTunnelNetworkRegistry::new();
        let t1 = ObjectId(10);
        let t2 = ObjectId(20);
        let u1 = ObjectId(1);
        let u2 = ObjectId(2);

        assert!(reg.has_capacity(Team::GLA));
        assert!(reg.record_enter(Team::GLA, u1, t1));
        assert_eq!(reg.contain_count(Team::GLA), 1);
        assert_eq!(reg.entry_tunnel_of(u1), Some(t1));
        assert!(reg.honesty_enter_exit_ok() == false); // no exit yet

        // Cross exit at t2.
        assert_eq!(reg.record_exit(Team::GLA, u1, t2), Some(t1));
        assert!(reg.honesty_enter_exit_ok());
        assert!(reg.honesty_cross_exit_ok());
        assert_eq!(reg.contain_count(Team::GLA), 0);

        // Capacity fills to MAX.
        for i in 0..MAX_TUNNEL_CAPACITY {
            assert!(reg.record_enter(Team::GLA, ObjectId(100 + i as u32), t1));
        }
        assert!(!reg.has_capacity(Team::GLA));
        assert!(!reg.record_enter(Team::GLA, u2, t1));
    }

    #[test]
    fn aircraft_rejected_from_tunnel() {
        assert!(!unit_can_use_tunnel(true, true, false));
        assert!(unit_can_use_tunnel(false, true, false));
        assert!(!unit_can_use_tunnel(false, false, false));
        assert!(!unit_can_use_tunnel(false, true, true));
    }

    #[test]
    fn same_tunnel_exit_is_not_cross() {
        let mut reg = HostTunnelNetworkRegistry::new();
        let t1 = ObjectId(10);
        let u1 = ObjectId(1);
        reg.record_enter(Team::GLA, u1, t1);
        reg.record_exit(Team::GLA, u1, t1);
        assert!(reg.honesty_enter_exit_ok());
        assert!(!reg.honesty_cross_exit_ok());
    }

    #[test]
    fn tunnel_network_residual_pack_honesty() {
        assert_eq!(tunnel_network_ms_to_frames(250), 8);
        assert_eq!(tunnel_network_ms_to_frames(500), 15);
        assert_eq!(tunnel_network_ms_to_frames(2500), 75);
        assert_eq!(tunnel_network_ms_to_frames(5000), 150);
        assert!(honesty_tunnel_network_gun_residual_ok());
        assert!(honesty_tunnel_network_contain_residual_ok());
        assert!(honesty_tunnel_network_body_residual_ok());
        assert!(honesty_tunnel_network_detector_spawn_residual_ok());
        assert!(honesty_tunnel_network_camo_hole_residual_ok());
        assert!(honesty_tunnel_network_residual_pack_ok());
    }
}
