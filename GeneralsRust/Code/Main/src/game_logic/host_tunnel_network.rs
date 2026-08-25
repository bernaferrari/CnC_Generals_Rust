//! Host GLA Tunnel Network residual.
//!
//! Residual slice (playability):
//! - `TunnelContain` shared passenger pool per **Player** (`Player::m_tunnelSystem`,
//!   `GameData.ini MaxTunnelCapacity = 10`). Faction `Team` is not a player.
//! - Enter any of **this player's** tunnel network structures
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
//! - Not CaveSystem multi-index (player TunnelTracker last-tunnel cave-in IS live)
//! - ExitStart/End walk is OpenContain inherit (default NumberOfExitPaths=1)
//! - Sneak-attack PRIMARY is `TunnelNetworkGunDUMMY` (0.01 / 175 / 1000ms)
//! - Not network tunnel-network replication (network deferred)

use super::{ObjectId, Team, Weapon};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Logic frames per second (host fixed step).
pub const TUNNEL_NETWORK_LOGIC_FPS: f32 = 30.0;

/// C++ `GameData.ini` `MaxTunnelCapacity = 10`.
pub const MAX_TUNNEL_CAPACITY: usize = 10;

/// Residual of TunnelContain `TimeForFullHeal = 5000` ms.
pub const TUNNEL_FULL_HEAL_MS: u32 = 5000;
/// Residual of TunnelContain `TimeForFullHeal = 5000` ms → frames @ 30 FPS.
/// C++ TunnelTracker::healObjects uses this duration for sliver + snap-to-max.
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
/// Retail sneak-attack PRIMARY `TunnelNetworkGunDUMMY`.
pub const TUNNEL_NETWORK_GUN_DUMMY: &str = "TunnelNetworkGunDUMMY";
/// Retail TunnelNetworkGunDUMMY PrimaryDamage (AI Guard acquire only).
pub const TUNNEL_NETWORK_GUN_DUMMY_DAMAGE: f32 = 0.01;
/// Retail TunnelNetworkGunDUMMY AttackRange.
pub const TUNNEL_NETWORK_GUN_DUMMY_RANGE: f32 = 175.0;
/// Retail TunnelNetworkGunDUMMY DelayBetweenShots residual (msec).
pub const TUNNEL_NETWORK_GUN_DUMMY_DELAY_MS: u32 = 1000;
/// Retail DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const TUNNEL_NETWORK_GUN_DUMMY_DELAY_FRAMES: u32 = 30;

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

/// Host residual honesty counters + per-player shared contain state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostTunnelNetworkRegistry {
    /// Successful residual enters into any player tunnel.
    pub enters: u32,
    /// Successful residual exits (same or cross tunnel).
    pub exits: u32,
    /// Exits where exit tunnel != entry tunnel (the key residual path).
    pub cross_exits: u32,
    /// Residual TunnelNetworkGun auto-fire honesty shots.
    pub gun_fires: u32,
    /// Residual units hit by TunnelNetworkGun residual.
    pub gun_units_hit: u32,
    /// C++ TunnelTracker::healObjects ticks applied (sliver or snap-to-max).
    pub heal_ticks: u32,
    /// C++ HealContain::update auto-exits after TimeForFullHeal.
    pub heal_auto_exits: u32,
    /// C++ `TunnelTracker::onTunnelDestroyed` invocations.
    pub tunnels_destroyed: u32,
    /// C++ last-tunnel cave-in (`m_tunnelCount == 0`) events.
    pub cave_ins: u32,
    /// Units destroyed by last-tunnel cave-in (`TunnelTracker::destroyObject`).
    pub cave_in_kills: u32,
    /// C++ `Object::m_containedByFrame` residual for HealContain + TunnelTracker.
    contained_by_frame: HashMap<u32, u32>,
    /// Per-player shared passenger lists (C++ `Player::m_tunnelSystem`).
    networks: HashMap<u32, PlayerTunnelNetwork>,
    /// C++ SpawnBehavior OneShot latch (`m_oneShotCountdown` exhausted / stopSpawning).
    /// Keyed by tunnel ObjectId bits so dying children cannot re-arm OneShot.
    #[serde(default)]
    oneshot_spawn_fired: HashSet<u32>,
    /// Units that sallied out via AITNGuard idle (C++ return-to-tunnel loop).
    #[serde(default)]
    sally_units: HashSet<u32>,
}

/// Shared contain state for one player's tunnel network.
///
/// C++ `Player::m_tunnelSystem` (`Player.cpp` init + `getTunnelSystem`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerTunnelNetwork {
    /// Units currently inside the communal tunnel pool.
    pub contained: Vec<ObjectId>,
    /// Unit → tunnel they entered (scripts / cross-exit honesty).
    pub entry_tunnel: HashMap<u32, ObjectId>,
    /// C++ `TunnelTracker::m_tunnelIDs` residual for this player.
    pub tunnel_ids: Vec<ObjectId>,
    /// C++ `TunnelTracker::m_curNemesisID`.
    #[serde(default)]
    pub cur_nemesis: Option<ObjectId>,
    /// C++ `TunnelTracker::m_nemesisTimestamp`.
    #[serde(default)]
    pub nemesis_timestamp: u32,
}

/// High bit so ownerless test objects never collide with live player ids.
pub const TUNNEL_UNOWNED_TEAM_BASE: u32 = 0x8000_0000;

/// C++ `Player::m_tunnelSystem` key: controlling player id.
///
/// Ownerless residuals (legacy tests / Neutral) fall back to a faction
/// sentinel so they stay isolated from real player slots.
#[inline]
pub fn tunnel_system_key(owner_player_id: Option<u32>, team: Team) -> u32 {
    match owner_player_id {
        Some(id) => id,
        None => TUNNEL_UNOWNED_TEAM_BASE | (team as u32),
    }
}

/// C++ `TunnelTracker::onTunnelDestroyed` result.
#[derive(Debug, Clone, Default)]
pub struct TunnelDestroyedOutcome {
    /// True when this was the last registered tunnel (`m_tunnelCount == 0`).
    pub cave_in: bool,
    /// Shared-pool units that must be destroyed (cave-in only).
    pub cave_in_units: Vec<ObjectId>,
    /// Remaining valid tunnel for remapping `ContainedBy` (non-last).
    pub remapped_to: Option<ObjectId>,
}

impl HostTunnelNetworkRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the live per-player TunnelTracker pool from a snapshot tail.
    pub fn restore_from(&mut self, other: Self) {
        *self = other;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// C++ OneShot SpawnBehavior already delivered its batch for this tunnel.
    pub fn oneshot_spawn_fired(&self, tunnel_id: ObjectId) -> bool {
        self.oneshot_spawn_fired.contains(&tunnel_id.0)
    }

    /// Latch OneShot so a later empty child list cannot spawn again.
    pub fn mark_oneshot_spawn_fired(&mut self, tunnel_id: ObjectId) {
        self.oneshot_spawn_fired.insert(tunnel_id.0);
    }

    pub fn network(&self, player_id: u32) -> Option<&PlayerTunnelNetwork> {
        self.networks.get(&player_id)
    }

    pub fn network_mut(&mut self, player_id: u32) -> &mut PlayerTunnelNetwork {
        self.networks.entry(player_id).or_default()
    }

    pub fn contain_count(&self, player_id: u32) -> usize {
        self.networks
            .get(&player_id)
            .map(|n| n.contained.len())
            .unwrap_or(0)
    }

    pub fn has_capacity(&self, player_id: u32) -> bool {
        self.contain_count(player_id) < MAX_TUNNEL_CAPACITY
    }

    pub fn is_in_network(&self, player_id: u32, unit_id: ObjectId) -> bool {
        self.networks
            .get(&player_id)
            .map(|n| n.contained.contains(&unit_id))
            .unwrap_or(false)
    }

    /// Find which player's tracker currently holds this unit.
    pub fn player_holding_unit(&self, unit_id: ObjectId) -> Option<u32> {
        for (player_id, net) in &self.networks {
            if net.contained.contains(&unit_id) {
                return Some(*player_id);
            }
        }
        None
    }

    /// Find which player's tracker currently lists this entrance.
    pub fn player_holding_tunnel(&self, tunnel_id: ObjectId) -> Option<u32> {
        for (player_id, net) in &self.networks {
            if net.tunnel_ids.contains(&tunnel_id) {
                return Some(*player_id);
            }
        }
        None
    }

    /// Player ids whose communal pool is non-empty.
    pub fn occupant_player_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self
            .networks
            .iter()
            .filter(|(_, net)| !net.contained.is_empty())
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// C++ `TunnelTracker::getContainerList` residual.
    pub fn tunnel_ids_for(&self, player_id: u32) -> &[ObjectId] {
        self.networks
            .get(&player_id)
            .map(|n| n.tunnel_ids.as_slice())
            .unwrap_or(&[])
    }

    /// C++ AITNGuardIdleState sally set (units that must return when idle).
    pub fn sally_unit_ids(&self) -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> = self.sally_units.iter().copied().map(ObjectId).collect();
        ids.sort_unstable_by_key(|id| id.0);
        ids
    }

    pub fn mark_sally(&mut self, unit_id: ObjectId) {
        self.sally_units.insert(unit_id.0);
    }

    pub fn clear_sally(&mut self, unit_id: ObjectId) {
        self.sally_units.remove(&unit_id.0);
    }

    pub fn entry_tunnel_of(&self, unit_id: ObjectId) -> Option<ObjectId> {
        for net in self.networks.values() {
            if let Some(&tid) = net.entry_tunnel.get(&unit_id.0) {
                return Some(tid);
            }
        }
        None
    }

    /// List all unit IDs currently in the player's shared tunnel pool.
    pub fn contained_for_player(&self, player_id: u32) -> Vec<ObjectId> {
        self.networks
            .get(&player_id)
            .map(|n| n.contained.clone())
            .unwrap_or_default()
    }

    /// Record enter into shared pool at `entry_tunnel`.
    /// Returns false if capacity full. If already contained, keeps original
    /// entry tunnel (cross-exit honesty: enter A then leave via B).
    pub fn record_enter(
        &mut self,
        player_id: u32,
        unit_id: ObjectId,
        entry_tunnel: ObjectId,
    ) -> bool {
        if self.is_in_network(player_id, unit_id) {
            // Already in shared pool (transfer residual between entrances).
            // Preserve original entry tunnel for cross-exit honesty.
            let _ = entry_tunnel;
            return true;
        }
        if !self.has_capacity(player_id) {
            return false;
        }
        let net = self.network_mut(player_id);
        net.contained.push(unit_id);
        net.entry_tunnel.insert(unit_id.0, entry_tunnel);
        self.enters = self.enters.saturating_add(1);
        self.sally_units.remove(&unit_id.0);

        true
    }

    /// Remove unit from shared pool. Returns entry tunnel if it was contained.
    pub fn record_exit(
        &mut self,
        player_id: u32,
        unit_id: ObjectId,
        exit_tunnel: ObjectId,
    ) -> Option<ObjectId> {
        let net = self.networks.get_mut(&player_id)?;
        let pos = net.contained.iter().position(|&id| id == unit_id)?;
        net.contained.remove(pos);
        let entry = net.entry_tunnel.remove(&unit_id.0);
        self.exits = self.exits.saturating_add(1);
        if let Some(entry_id) = entry {
            if entry_id != exit_tunnel {
                self.cross_exits = self.cross_exits.saturating_add(1);
            }
        }
        self.contained_by_frame.remove(&unit_id.0);
        entry
    }

    /// C++ `TunnelTracker::onTunnelCreated`.
    pub fn on_tunnel_created(&mut self, player_id: u32, tunnel: ObjectId) {
        let net = self.network_mut(player_id);
        if !net.tunnel_ids.contains(&tunnel) {
            net.tunnel_ids.push(tunnel);
        }
    }

    /// Live tunnel count residual (`TunnelTracker::m_tunnelCount` / `friend_getTunnelCount`).
    pub fn tunnel_count(&self, player_id: u32) -> usize {
        self.networks
            .get(&player_id)
            .map(|n| n.tunnel_ids.len())
            .unwrap_or(0)
    }

    /// C++ `TunnelTracker::onTunnelDestroyed`.
    ///
    /// `remaining_other_tunnels` is the live player tunnel list after `dead_tunnel`
    /// is already gone (host scans objects). Last tunnel cave-in kills the
    /// shared pool via `record_exit` + returned IDs; otherwise occupants stay
    /// and entry tunnels that pointed at the dead entrance remap to
    /// `m_tunnelIDs.front()` (oldest registered surviving entrance), not the
    /// HashMap scan order of `remaining_other_tunnels`.
    pub fn on_tunnel_destroyed(
        &mut self,
        player_id: u32,
        dead_tunnel: ObjectId,
        remaining_other_tunnels: &[ObjectId],
    ) -> TunnelDestroyedOutcome {
        self.tunnels_destroyed = self.tunnels_destroyed.saturating_add(1);
        if let Some(net) = self.networks.get_mut(&player_id) {
            net.tunnel_ids.retain(|id| *id != dead_tunnel);
        }
        if remaining_other_tunnels.is_empty() {
            let units = self.contained_for_player(player_id);
            for &uid in &units {
                let _ = self.record_exit(player_id, uid, dead_tunnel);
            }
            self.cave_ins = self.cave_ins.saturating_add(1);
            self.cave_in_kills = self.cave_in_kills.saturating_add(units.len() as u32);
            TunnelDestroyedOutcome {
                cave_in: true,
                cave_in_units: units,
                remapped_to: None,
            }
        } else {
            let remapped_to = {
                let registered = self
                    .networks
                    .get(&player_id)
                    .map(|n| n.tunnel_ids.as_slice())
                    .unwrap_or(&[]);
                oldest_surviving_tunnel(registered, remaining_other_tunnels)
                    .unwrap_or(remaining_other_tunnels[0])
            };
            if let Some(net) = self.networks.get_mut(&player_id) {
                for entry in net.entry_tunnel.values_mut() {
                    if *entry == dead_tunnel {
                        *entry = remapped_to;
                    }
                }
            }
            TunnelDestroyedOutcome {
                cave_in: false,
                cave_in_units: Vec::new(),
                remapped_to: Some(remapped_to),
            }
        }
    }

    /// Residual honesty: last-tunnel cave-in destroyed at least one contained unit.
    pub fn honesty_cave_in_ok(&self) -> bool {
        self.cave_ins > 0 && self.cave_in_kills > 0
    }

    /// C++ `Object::setContainedBy` stamps `m_containedByFrame`.
    pub fn stamp_contained_by_frame(&mut self, unit_id: ObjectId, frame: u32) {
        self.contained_by_frame.insert(unit_id.0, frame);
    }

    /// C++ `Object::getContainedByFrame`.
    pub fn contained_by_frame(&self, unit_id: ObjectId) -> Option<u32> {
        self.contained_by_frame.get(&unit_id.0).copied()
    }

    pub fn clear_contained_by_frame(&mut self, unit_id: ObjectId) {
        self.contained_by_frame.remove(&unit_id.0);
    }

    pub fn restore_contained_by_frames(&mut self, frames: &[(ObjectId, u32)]) {
        self.contained_by_frame.clear();
        for (unit_id, frame) in frames {
            self.contained_by_frame.insert(unit_id.0, *frame);
        }
    }

    pub fn record_heal_tick(&mut self) {
        self.heal_ticks = self.heal_ticks.saturating_add(1);
    }

    pub fn record_heal_auto_exit(&mut self) {
        self.heal_auto_exits = self.heal_auto_exits.saturating_add(1);
    }

    /// Residual honesty: TunnelTracker::healObjects exercised.
    pub fn honesty_heal_objects_ok(&self) -> bool {
        self.heal_ticks > 0
    }

    /// C++ `TunnelTracker::updateNemesis` (TunnelTracker.cpp:87-100).
    /// Sets only when empty; refreshes timestamp when the same target is seen.
    pub fn update_nemesis(
        &mut self,
        player_id: u32,
        target: ObjectId,
        is_vehicle: bool,
        is_structure: bool,
        is_infantry: bool,
        is_aircraft: bool,
        frame: u32,
    ) {
        if !(is_vehicle || is_structure || is_infantry || is_aircraft) {
            return;
        }
        let net = self.network_mut(player_id);
        match net.cur_nemesis {
            None => {
                net.cur_nemesis = Some(target);
                net.nemesis_timestamp = frame;
            }
            Some(cur) if cur == target => {
                net.nemesis_timestamp = frame;
            }
            Some(_) => {}
        }
    }

    /// C++ `TunnelTracker::getCurNemesis` timeout half (TunnelTracker.cpp:103-108).
    /// Caller must still reject stealthed-undetected / effectively-dead targets.
    pub fn get_cur_nemesis_id(&mut self, player_id: u32, frame: u32) -> Option<ObjectId> {
        const NEMESIS_TIMEOUT_FRAMES: u32 = 4 * 30;
        let net = self.networks.get_mut(&player_id)?;
        let id = net.cur_nemesis?;
        if net.nemesis_timestamp.saturating_add(NEMESIS_TIMEOUT_FRAMES) < frame {
            net.cur_nemesis = None;
            return None;
        }
        Some(id)
    }

    /// Clear an expired / invalid nemesis (stealthed, dead, missing).
    pub fn clear_nemesis(&mut self, player_id: u32) {
        if let Some(net) = self.networks.get_mut(&player_id) {
            net.cur_nemesis = None;
        }
    }

    /// Residual honesty: HealContain auto-exit exercised.
    pub fn honesty_heal_contain_auto_exit_ok(&self) -> bool {
        self.heal_auto_exits > 0
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

/// C++ `findBestTunnel` (AITNGuard.cpp:84-105): closest living tunnel to `pos`
/// in the ground plane (`dx*dx+dy*dy`). Host ground is XZ (y is height).
pub fn find_best_tunnel_xz(
    tunnels: impl IntoIterator<Item = (ObjectId, f32, f32)>,
    pos_x: f32,
    pos_z: f32,
) -> Option<ObjectId> {
    let mut best: Option<(ObjectId, f32)> = None;
    for (id, x, z) in tunnels {
        let dx = x - pos_x;
        let dz = z - pos_z;
        let dist_sqr = dx * dx + dz * dz;
        if best.as_ref().map_or(true, |(_, d)| dist_sqr < *d) {
            best = Some((id, dist_sqr));
        }
    }
    best.map(|(id, _)| id)
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
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: TUNNEL_NETWORK_GUN_WEAPON_SPEED,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
    }
}

/// Build residual TunnelNetworkGunDUMMY (sneak-attack PRIMARY).
pub fn tunnel_network_gun_dummy_weapon() -> Weapon {
    Weapon {
        damage: TUNNEL_NETWORK_GUN_DUMMY_DAMAGE,
        range: TUNNEL_NETWORK_GUN_DUMMY_RANGE,
        min_range: 0.0,
        reload_time: TUNNEL_NETWORK_GUN_DUMMY_DELAY_FRAMES as f32 / TUNNEL_NETWORK_LOGIC_FPS,
        last_fire_time: 0.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: TUNNEL_NETWORK_GUN_WEAPON_SPEED,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
    }
}

/// True when template is a sneak-attack Tunnel Network (not Start/Hole).
pub fn is_sneak_attack_tunnel_template(template_name: &str) -> bool {
    is_tunnel_network_template(template_name)
        && template_name.to_ascii_lowercase().contains("sneak")
}

/// Retail PRIMARY weapon for this tunnel template.
pub fn tunnel_network_primary_weapon(template_name: &str) -> Weapon {
    if is_sneak_attack_tunnel_template(template_name) {
        tunnel_network_gun_dummy_weapon()
    } else {
        tunnel_network_gun_weapon()
    }
}

/// Retail PRIMARY weapon template name for this tunnel.
pub fn tunnel_network_primary_weapon_name(template_name: &str) -> &'static str {
    if is_sneak_attack_tunnel_template(template_name) {
        TUNNEL_NETWORK_GUN_DUMMY
    } else {
        TUNNEL_NETWORK_GUN
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

/// True when this Tunnel Network authors the free OneShot RPG-trooper batch.
///
/// Sneak-attack tunnels share the contain residual but do not OneShot-spawn
/// `GLAInfantryTunnelDefender`.
pub fn tunnel_network_has_oneshot_spawn(template_name: &str) -> bool {
    is_tunnel_network_template(template_name)
        && !template_name.to_ascii_lowercase().contains("sneak")
}

/// Whether residual spawn should install `StealthDetectorUpdate` fields.
///
/// C++ `GLATunnelNetwork` ModuleTag_13: DetectionRange **150**, DetectionRate **500**ms.
/// Sneak-attack tunnels share TunnelContain but omit the detector module.
pub fn tunnel_network_spawn_is_detector(template_name: &str) -> bool {
    is_tunnel_network_template(template_name)
        && !template_name.to_ascii_lowercase().contains("sneak")
}

/// Retail StealthDetectorUpdate DetectionRange, or `None` if this template is not a detector.
pub fn tunnel_network_detection_range(template_name: &str) -> Option<f32> {
    if tunnel_network_spawn_is_detector(template_name) {
        Some(TUNNEL_NETWORK_DETECTION_RANGE)
    } else {
        None
    }
}

/// Prefix a stock spawn template with the building's general prefix.
pub fn general_prefixed_spawn_template(building: &str, stock: &str) -> String {
    const PREFIXES: &[&str] = &["GC_Slth_", "GC_Chem_", "Demo_", "Chem_", "Slth_"];
    for prefix in PREFIXES {
        if building.len() >= prefix.len() && building[..prefix.len()].eq_ignore_ascii_case(prefix) {
            return format!("{prefix}{stock}");
        }
    }
    stock.to_string()
}

/// Retail SpawnTemplateName for a Tunnel Network (general-prefixed when needed).
pub fn tunnel_network_spawn_template_for(template_name: &str) -> String {
    general_prefixed_spawn_template(template_name, TUNNEL_NETWORK_SPAWN_TEMPLATE)
}

/// C++ TunnelTracker::isValidContainerFor residual: reject aircraft only.
pub fn unit_can_use_tunnel(is_aircraft: bool, is_alive: bool, under_construction: bool) -> bool {
    is_alive && !under_construction && !is_aircraft
}

/// C++ TunnelTracker::healObject / HealContain::doHeal amount for one frame.
///
/// If `contained_frames >= frames_for_full_heal` (including a 0-frame HealContain
/// default), snap to max health. Otherwise apply `max_health / frames`.
pub fn tunnel_tracker_heal_amount(
    max_health: f32,
    contained_frames: u32,
    frames_for_full_heal: u32,
) -> f32 {
    if max_health <= 0.0 {
        return 0.0;
    }
    if contained_frames >= frames_for_full_heal {
        max_health
    } else {
        max_health / (frames_for_full_heal as f32)
    }
}

/// C++ HealContain::doHeal done flag: contained duration reached TimeForFullHeal.
/// A 0-frame authored duration completes on the first update (`contained >= 0`).
pub fn heal_contain_done(contained_frames: u32, frames_for_full_heal: u32) -> bool {
    contained_frames >= frames_for_full_heal
}

/// C++ `TunnelTracker::onTunnelDestroyed` remaps to `m_tunnelIDs.front()`
/// (oldest registered surviving entrance). Fall back to the smallest ObjectId
/// among `remaining` when registration was missed (IDs are monotonic).
fn oldest_surviving_tunnel(registered: &[ObjectId], remaining: &[ObjectId]) -> Option<ObjectId> {
    registered
        .iter()
        .copied()
        .find(|id| remaining.iter().any(|r| r == id))
        .or_else(|| remaining.iter().copied().min_by_key(|id| id.0))
}

#[cfg(test)]
mod tests {
    fn gla_key() -> u32 {
        tunnel_system_key(None, Team::GLA)
    }

    use super::*;

    #[test]
    fn template_detection_matches_gla_and_variants() {
        assert!(is_tunnel_network_template("GLATunnelNetwork"));
        assert!(is_tunnel_network_template("GLASneakAttackTunnelNetwork"));
        assert!(is_tunnel_network_template("Demo_GLATunnelNetwork"));
        assert!(is_tunnel_network_template("Chem_GLATunnelNetwork"));
        assert!(is_tunnel_network_template("TestTunnelNetwork"));
        assert!(!is_tunnel_network_template("GLAHoleTunnelNetwork"));
        assert!(!is_tunnel_network_template(
            "GLASneakAttackTunnelNetworkStart"
        ));
        assert!(!is_tunnel_network_template("GLATunnelNetworkNoSpawn"));
        assert!(!is_tunnel_network_template("GLA_Barracks"));
        assert!(!is_tunnel_network_template("TestBunker"));
    }

    #[test]
    fn find_best_tunnel_xz_picks_closest_to_nemesis() {
        // C++ findBestTunnel(nemesis pos): entry far, exit near nemesis.
        let entry = ObjectId(1);
        let near = ObjectId(2);
        let far = ObjectId(3);
        let best = find_best_tunnel_xz(
            [(entry, 0.0, 0.0), (near, 100.0, 0.0), (far, 400.0, 0.0)],
            110.0,
            0.0,
        );
        assert_eq!(best, Some(near));
    }

    #[test]
    fn record_enter_clears_sally_mark() {
        let mut reg = HostTunnelNetworkRegistry::new();
        let u = ObjectId(9);
        let t = ObjectId(1);
        reg.mark_sally(u);
        assert_eq!(reg.sally_unit_ids(), vec![u]);
        assert!(reg.record_enter(gla_key(), u, t));
        assert!(reg.sally_unit_ids().is_empty());
    }

    #[test]
    fn oneshot_spawn_excludes_sneak_and_prefixes_generals() {
        assert!(tunnel_network_has_oneshot_spawn("GLATunnelNetwork"));
        assert!(tunnel_network_has_oneshot_spawn("Demo_GLATunnelNetwork"));
        assert!(!tunnel_network_has_oneshot_spawn(
            "GLASneakAttackTunnelNetwork"
        ));
        assert_eq!(
            tunnel_network_spawn_template_for("GLATunnelNetwork"),
            "GLAInfantryTunnelDefender"
        );
        assert_eq!(
            tunnel_network_spawn_template_for("Demo_GLATunnelNetwork"),
            "Demo_GLAInfantryTunnelDefender"
        );
        let mut reg = HostTunnelNetworkRegistry::new();
        let id = ObjectId(7);
        assert!(!reg.oneshot_spawn_fired(id));
        reg.mark_oneshot_spawn_fired(id);
        assert!(reg.oneshot_spawn_fired(id));
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

        assert!(reg.has_capacity(gla_key()));
        assert!(reg.record_enter(gla_key(), u1, t1));
        assert_eq!(reg.contain_count(gla_key()), 1);
        assert_eq!(reg.entry_tunnel_of(u1), Some(t1));
        assert!(reg.honesty_enter_exit_ok() == false); // no exit yet

        // Cross exit at t2.
        assert_eq!(reg.record_exit(gla_key(), u1, t2), Some(t1));
        assert!(reg.honesty_enter_exit_ok());
        assert!(reg.honesty_cross_exit_ok());
        assert_eq!(reg.contain_count(gla_key()), 0);

        // Capacity fills to MAX.
        for i in 0..MAX_TUNNEL_CAPACITY {
            assert!(reg.record_enter(gla_key(), ObjectId(100 + i as u32), t1));
        }
        assert!(!reg.has_capacity(gla_key()));
        assert!(!reg.record_enter(gla_key(), u2, t1));
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
        reg.record_enter(gla_key(), u1, t1);
        reg.record_exit(gla_key(), u1, t1);
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

    #[test]
    fn tunnel_tracker_heal_object_matches_cpp() {
        // C++ TunnelTracker.cpp:248-268 / HealContain.cpp:127-151.
        assert!((tunnel_tracker_heal_amount(150.0, 0, 150) - 1.0).abs() < 1e-5);
        assert!((tunnel_tracker_heal_amount(150.0, 149, 150) - 1.0).abs() < 1e-5);
        assert!((tunnel_tracker_heal_amount(150.0, 150, 150) - 150.0).abs() < 1e-5);
        // HealContain default TimeForFullHeal = 0 → first update snaps to max.
        assert!((tunnel_tracker_heal_amount(80.0, 0, 0) - 80.0).abs() < 1e-5);
        assert!(heal_contain_done(0, 0));
        assert!(!heal_contain_done(59, 60));
        assert!(heal_contain_done(60, 60));

        let mut reg = HostTunnelNetworkRegistry::new();
        let unit = ObjectId(7);
        reg.stamp_contained_by_frame(unit, 10);
        assert_eq!(reg.contained_by_frame(unit), Some(10));
        reg.record_heal_tick();
        assert!(reg.honesty_heal_objects_ok());
        assert!(!reg.honesty_heal_contain_auto_exit_ok());
        reg.record_heal_auto_exit();
        assert!(reg.honesty_heal_contain_auto_exit_ok());
        reg.clear_contained_by_frame(unit);
        assert_eq!(reg.contained_by_frame(unit), None);
    }

    #[test]
    fn last_tunnel_cave_in_kills_shared_pool() {
        // C++ TunnelTracker.cpp:187-197 / destroyObject:215-220.
        let mut reg = HostTunnelNetworkRegistry::new();
        let t1 = ObjectId(10);
        let u1 = ObjectId(1);
        let u2 = ObjectId(2);
        reg.on_tunnel_created(gla_key(), t1);
        assert!(reg.record_enter(gla_key(), u1, t1));
        assert!(reg.record_enter(gla_key(), u2, t1));
        let out = reg.on_tunnel_destroyed(gla_key(), t1, &[]);
        assert!(out.cave_in);
        assert_eq!(out.cave_in_units, vec![u1, u2]);
        assert_eq!(reg.contain_count(gla_key()), 0);
        assert!(reg.honesty_cave_in_ok());
        assert_eq!(reg.exits, 2);
    }

    #[test]
    fn non_last_tunnel_keeps_pool_and_remaps_entry() {
        // C++ TunnelTracker.cpp:199-210 — no eject; remapped ContainedBy.
        let mut reg = HostTunnelNetworkRegistry::new();
        let t1 = ObjectId(10);
        let t2 = ObjectId(20);
        let u1 = ObjectId(1);
        reg.on_tunnel_created(gla_key(), t1);
        reg.on_tunnel_created(gla_key(), t2);
        assert!(reg.record_enter(gla_key(), u1, t1));
        let out = reg.on_tunnel_destroyed(gla_key(), t1, &[t2]);
        assert!(!out.cave_in);
        assert!(out.cave_in_units.is_empty());
        assert_eq!(out.remapped_to, Some(t2));
        assert_eq!(reg.contain_count(gla_key()), 1);
        assert!(reg.is_in_network(gla_key(), u1));
        assert_eq!(reg.entry_tunnel_of(u1), Some(t2));
        assert!(!reg.honesty_cave_in_ok());
    }

    #[test]
    fn non_last_tunnel_remaps_to_oldest_registered_not_scan_front() {
        // C++ TunnelTracker.cpp:201 — m_tunnelIDs.front() after remove, not
        // remaining_other_tunnels[0] HashMap-scan order.
        let mut reg = HostTunnelNetworkRegistry::new();
        let t1 = ObjectId(10);
        let t2 = ObjectId(20);
        let t3 = ObjectId(30);
        let u1 = ObjectId(1);
        reg.on_tunnel_created(gla_key(), t1);
        reg.on_tunnel_created(gla_key(), t2);
        reg.on_tunnel_created(gla_key(), t3);
        assert!(reg.record_enter(gla_key(), u1, t1));
        let out = reg.on_tunnel_destroyed(gla_key(), t1, &[t3, t2]);
        assert!(!out.cave_in);
        assert_eq!(out.remapped_to, Some(t2), "oldest surviving registered");
        assert_eq!(reg.entry_tunnel_of(u1), Some(t2));
    }

    #[test]
    fn sneak_attack_uses_dummy_gun_not_real_mg() {
        assert!(is_sneak_attack_tunnel_template(
            "GLASneakAttackTunnelNetwork"
        ));
        assert!(!is_sneak_attack_tunnel_template("GLATunnelNetwork"));
        assert!(!is_sneak_attack_tunnel_template(
            "GLASneakAttackTunnelNetworkStart"
        ));
        assert_eq!(
            tunnel_network_primary_weapon_name("GLASneakAttackTunnelNetwork"),
            TUNNEL_NETWORK_GUN_DUMMY
        );
        assert_eq!(
            tunnel_network_primary_weapon_name("GLATunnelNetwork"),
            TUNNEL_NETWORK_GUN
        );
        let dummy = tunnel_network_primary_weapon("GLASneakAttackTunnelNetwork");
        assert!((dummy.damage - 0.01).abs() < 1e-6);
        assert!((dummy.range - 175.0).abs() < 0.01);
        assert!((dummy.reload_time - 1.0).abs() < 0.001);
        let real = tunnel_network_primary_weapon("GLATunnelNetwork");
        assert!((real.damage - 15.0).abs() < 0.01);
        assert!((real.reload_time - (8.0 / 30.0)).abs() < 0.001);
    }

    #[test]
    fn update_nemesis_sets_once_and_refreshes_same_target() {
        let mut reg = HostTunnelNetworkRegistry::new();
        let a = ObjectId(50);
        let b = ObjectId(51);
        reg.update_nemesis(gla_key(), a, true, false, false, false, 10);
        assert_eq!(reg.get_cur_nemesis_id(gla_key(), 10), Some(a));
        // Different target while current is live does not replace (C++).
        reg.update_nemesis(gla_key(), b, false, false, true, false, 20);
        assert_eq!(reg.get_cur_nemesis_id(gla_key(), 20), Some(a));
        reg.update_nemesis(gla_key(), a, true, false, false, false, 30);
        assert_eq!(reg.get_cur_nemesis_id(gla_key(), 30), Some(a));
        // 4 seconds after last refresh expires.
        assert_eq!(reg.get_cur_nemesis_id(gla_key(), 30 + 4 * 30 + 1), None);
    }

    #[test]
    fn two_gla_players_do_not_share_a_pool() {
        // C++ Player.cpp m_tunnelSystem is per Player, not faction Team.
        // Demo vs Chem (or two GLA skirmish slots) each have MaxTunnelCapacity.
        let mut reg = HostTunnelNetworkRegistry::new();
        let demo = tunnel_system_key(Some(2), Team::GLA);
        let chem = tunnel_system_key(Some(3), Team::GLA);
        assert_ne!(demo, chem);
        let t_demo = ObjectId(10);
        let t_chem = ObjectId(20);
        reg.on_tunnel_created(demo, t_demo);
        reg.on_tunnel_created(chem, t_chem);
        for i in 0..MAX_TUNNEL_CAPACITY {
            assert!(reg.record_enter(demo, ObjectId(100 + i as u32), t_demo));
        }
        assert!(!reg.has_capacity(demo));
        assert!(reg.has_capacity(chem));
        assert!(reg.record_enter(chem, ObjectId(1), t_chem));
        assert_eq!(reg.contain_count(demo), MAX_TUNNEL_CAPACITY);
        assert_eq!(reg.contain_count(chem), 1);
        assert_eq!(reg.player_holding_unit(ObjectId(1)), Some(chem));
        assert_eq!(reg.player_holding_tunnel(t_demo), Some(demo));
    }
}
