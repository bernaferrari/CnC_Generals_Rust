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

/// C++ `GameData.ini` `MaxTunnelCapacity = 10`.
pub const MAX_TUNNEL_CAPACITY: usize = 10;

/// Residual of TunnelContain `TimeForFullHeal = 5000` ms → frames @ 30 FPS.
/// Fail-closed: heal tick not required for enter/exit honesty.
pub const TUNNEL_FULL_HEAL_FRAMES: u32 = 150;

/// Retail TunnelNetworkGun primary weapon template name.
pub const TUNNEL_NETWORK_GUN: &str = "TunnelNetworkGun";
/// Retail TunnelNetworkGun PrimaryDamage.
pub const TUNNEL_NETWORK_GUN_DAMAGE: f32 = 15.0;
/// Retail TunnelNetworkGun AttackRange.
pub const TUNNEL_NETWORK_GUN_RANGE: f32 = 175.0;
/// Retail DelayBetweenShots 250ms → 8 frames @ 30 FPS.
pub const TUNNEL_NETWORK_GUN_DELAY_FRAMES: u32 = 8;
/// Residual fire audio (retail FireSound = HumveeWeapon).
pub const TUNNEL_NETWORK_GUN_AUDIO: &str = "HumveeWeapon";

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
        reload_time: TUNNEL_NETWORK_GUN_DELAY_FRAMES as f32 / 30.0,
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 600.0,
        pre_attack_delay: 0.0,
    }
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
}
