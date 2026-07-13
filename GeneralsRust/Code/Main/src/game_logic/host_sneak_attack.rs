//! Host GLA Sneak Attack special-power residual — delayed tunnel spawn + shockwave.
//!
//! Residual slice (playability):
//! - `DoSpecialPower(SneakAttack)` at a world location queues a delayed tunnel
//!   spawn residual (retail SuperweaponSneakAttack → OCL_CreateSneakAttackTunnelStart
//!   → GLASneakAttackTunnelNetworkStart LifetimeUpdate 5000ms → CreateObjectDie
//!   OCL_CreateSneakAttackTunnel → GLASneakAttackTunnelNetwork).
//! - After residual Lifetime delay, a tunnel structure is created for the casting
//!   team at the target location, and a residual shockwave damage pulse is applied
//!   (retail FireWeaponUpdate SneakAttackShockwaveWeaponBig residual).
//! - Honesty counters/flags for residual gates and tests.
//!
//! Wave 62 residual pack (retail SpecialPower / Weapon / Tunnel residual):
//! - SuperweaponSneakAttack: ReloadTime **150000**ms → **4500**f, RadiusCursor **50**,
//!   RequiredScience **SCIENCE_SneakAttack**, SharedSyncedTimer **Yes**,
//!   InitiateAtLocationSound **SneakAttackActivated**
//! - Tunnel Start Lifetime **5000**ms → **150**f spawn delay; tunnel MaxHealth **1000**,
//!   Vision/Shroud **200**, template **GLASneakAttackTunnelNetwork**
//! - Multi-shockwave residual matrix:
//!   Small **10**/r**35** @ InitialDelay **10**ms → **1**f,
//!   Big **50**/r**50** @ **1000**ms → **30**f and **2500**ms → **75**f
//! - Host gameplay still collapses spawn to Big pulse residual (fail-closed multi-pulse
//!   live apply); honesty pack documents full INI timing matrix
//!
//! Fail-closed honesty:
//! - Not full OCL Start/Tunnel model animation / crack dust particle stack
//! - Not full multi-shockwave live damage apply (host still Big-only at spawn)
//! - TunnelContain enter/exit residual is host_tunnel_network (shared pool + cross-exit);
//!   not full GuardTunnelNetwork AI path
//! - Not SharedSyncedTimer / multiplayer academy classification
//! - Not network sneak-attack replication (network deferred)

use super::ObjectId;
use crate::command_system::SpecialPowerType;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const SNEAK_ATTACK_LOGIC_FPS: f32 = 30.0;

/// Retail SuperweaponSneakAttack RadiusCursorRadius residual (= 50).
pub const HOST_SNEAK_ATTACK_RADIUS: f32 = 50.0;

/// Retail GLASneakAttackTunnelNetworkStart LifetimeUpdate Min/MaxLifetime = 5000 ms.
pub const SNEAK_ATTACK_LIFETIME_MS: u32 = 5_000;
/// Tunnel spawn delay residual (Lifetime of Start object).
pub const SNEAK_ATTACK_SPAWN_DELAY_FRAMES: u32 = (SNEAK_ATTACK_LIFETIME_MS * 30) / 1000;

/// Residual of SneakAttackShockwaveWeaponBig PrimaryDamage.
pub const SNEAK_ATTACK_SHOCKWAVE_DAMAGE: f32 = 50.0;
/// Residual of SneakAttackShockwaveWeaponBig PrimaryDamageRadius.
pub const SNEAK_ATTACK_SHOCKWAVE_RADIUS: f32 = 50.0;

/// Preferred retail tunnel template after Start dies.
pub const GLA_SNEAK_TUNNEL_TEMPLATE: &str = "GLASneakAttackTunnelNetwork";
/// Residual structure template used when retail tunnel is unavailable.
pub const SNEAK_ATTACK_RESIDUAL_TEMPLATE: &str = "TestSneakTunnel";

/// Activate audio residual (SpecialPower.ini InitiateAtLocationSound).
pub const SNEAK_ATTACK_ACTIVATE_AUDIO: &str = "SneakAttackActivated";
/// Tunnel emerge / spawn audio residual.
pub const SNEAK_ATTACK_SPAWN_AUDIO: &str = "SneakAttackTunnelSpawn";

// --- Wave 62 special-power / tunnel / multi-shockwave residual pack ---

/// Retail SuperweaponSneakAttack ReloadTime (ms).
pub const SNEAK_ATTACK_RELOAD_TIME_MS: u32 = 150_000;
/// ReloadTime → frames @ 30 FPS (150000 / (1000/30) = 4500).
pub const SNEAK_ATTACK_RELOAD_TIME_FRAMES: u32 = 4500;
/// Retail RequiredScience residual.
pub const SNEAK_ATTACK_REQUIRED_SCIENCE: &str = "SCIENCE_SneakAttack";
/// Retail SpecialPower template name.
pub const SNEAK_ATTACK_SPECIAL_POWER_TEMPLATE: &str = "SuperweaponSneakAttack";
/// Retail SharedSyncedTimer residual.
pub const SNEAK_ATTACK_SHARED_SYNCED_TIMER: bool = true;
/// Retail ShortcutPower residual.
pub const SNEAK_ATTACK_SHORTCUT_POWER: bool = true;
/// Retail PublicTimer residual.
pub const SNEAK_ATTACK_PUBLIC_TIMER: bool = false;

/// Retail GLASneakAttackTunnelNetwork MaxHealth residual.
pub const SNEAK_ATTACK_TUNNEL_MAX_HEALTH: f32 = 1000.0;
/// Retail tunnel VisionRange / ShroudClearingRange residual.
pub const SNEAK_ATTACK_TUNNEL_VISION_RANGE: f32 = 200.0;
/// Retail OCL Start object residual name.
pub const SNEAK_ATTACK_TUNNEL_START_TEMPLATE: &str = "GLASneakAttackTunnelNetworkStart";
/// Retail OCL_CreateSneakAttackTunnelStart residual.
pub const SNEAK_ATTACK_OCL_START: &str = "OCL_CreateSneakAttackTunnelStart";
/// Retail OCL_CreateSneakAttackTunnel residual.
pub const SNEAK_ATTACK_OCL_TUNNEL: &str = "OCL_CreateSneakAttackTunnel";

/// Multi-shockwave residual entry (FireWeaponUpdate InitialDelay matrix).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SneakShockwavePulse {
    pub weapon_name: &'static str,
    pub primary_damage: f32,
    pub primary_radius: f32,
    pub initial_delay_ms: u32,
    pub initial_delay_frames: u32,
}

/// Retail SneakAttackShockwaveWeaponSmall residual.
pub const SNEAK_SHOCKWAVE_SMALL_DAMAGE: f32 = 10.0;
pub const SNEAK_SHOCKWAVE_SMALL_RADIUS: f32 = 35.0;
pub const SNEAK_SHOCKWAVE_SMALL_DELAY_MS: u32 = 10;
/// 10ms → 1 frame @ 30 FPS (ceil).
pub const SNEAK_SHOCKWAVE_SMALL_DELAY_FRAMES: u32 = 1;

/// Retail SneakAttackShockwaveWeaponBig residual (also Medium uses 50r / 30 dmg).
pub const SNEAK_SHOCKWAVE_BIG_DELAY_1_MS: u32 = 1000;
pub const SNEAK_SHOCKWAVE_BIG_DELAY_1_FRAMES: u32 = 30;
pub const SNEAK_SHOCKWAVE_BIG_DELAY_2_MS: u32 = 2500;
pub const SNEAK_SHOCKWAVE_BIG_DELAY_2_FRAMES: u32 = 75;
pub const SNEAK_SHOCKWAVE_MEDIUM_DAMAGE: f32 = 30.0;
pub const SNEAK_SHOCKWAVE_MEDIUM_RADIUS: f32 = 50.0;

/// Retail multi-shockwave residual matrix (Start object FireWeaponUpdate).
pub fn sneak_attack_shockwave_pulses() -> [SneakShockwavePulse; 3] {
    [
        SneakShockwavePulse {
            weapon_name: "SneakAttackShockwaveWeaponSmall",
            primary_damage: SNEAK_SHOCKWAVE_SMALL_DAMAGE,
            primary_radius: SNEAK_SHOCKWAVE_SMALL_RADIUS,
            initial_delay_ms: SNEAK_SHOCKWAVE_SMALL_DELAY_MS,
            initial_delay_frames: SNEAK_SHOCKWAVE_SMALL_DELAY_FRAMES,
        },
        SneakShockwavePulse {
            weapon_name: "SneakAttackShockwaveWeaponBig",
            primary_damage: SNEAK_ATTACK_SHOCKWAVE_DAMAGE,
            primary_radius: SNEAK_ATTACK_SHOCKWAVE_RADIUS,
            initial_delay_ms: SNEAK_SHOCKWAVE_BIG_DELAY_1_MS,
            initial_delay_frames: SNEAK_SHOCKWAVE_BIG_DELAY_1_FRAMES,
        },
        SneakShockwavePulse {
            weapon_name: "SneakAttackShockwaveWeaponBig",
            primary_damage: SNEAK_ATTACK_SHOCKWAVE_DAMAGE,
            primary_radius: SNEAK_ATTACK_SHOCKWAVE_RADIUS,
            initial_delay_ms: SNEAK_SHOCKWAVE_BIG_DELAY_2_MS,
            initial_delay_frames: SNEAK_SHOCKWAVE_BIG_DELAY_2_FRAMES,
        },
    ]
}

/// Absolute frame for a shockwave pulse after Start spawn at `activate_frame`.
pub fn sneak_shockwave_pulse_frame(activate_frame: u32, pulse: &SneakShockwavePulse) -> u32 {
    activate_frame.saturating_add(pulse.initial_delay_frames)
}

/// Host residual sneak-attack kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostSneakAttackKind {
    /// GLA SPECIAL_SNEAK_ATTACK / SuperweaponSneakAttack residual.
    GLASneakAttack,
}

impl HostSneakAttackKind {
    /// Map a command-system power type to a host residual sneak attack, if supported.
    pub fn from_command_power(power: &SpecialPowerType) -> Option<Self> {
        match power {
            SpecialPowerType::SneakAttack => Some(HostSneakAttackKind::GLASneakAttack),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HostSneakAttackKind::GLASneakAttack => "GLASneakAttack",
        }
    }

    /// Lifetime residual frames before tunnel spawns.
    pub fn spawn_delay_frames(self) -> u32 {
        match self {
            HostSneakAttackKind::GLASneakAttack => SNEAK_ATTACK_SPAWN_DELAY_FRAMES,
        }
    }

    /// Preferred retail tunnel template.
    pub fn tunnel_template(self) -> &'static str {
        match self {
            HostSneakAttackKind::GLASneakAttack => GLA_SNEAK_TUNNEL_TEMPLATE,
        }
    }

    /// Residual shockwave damage at spawn.
    pub fn shockwave_damage(self) -> f32 {
        match self {
            HostSneakAttackKind::GLASneakAttack => SNEAK_ATTACK_SHOCKWAVE_DAMAGE,
        }
    }

    /// Residual shockwave radius at spawn.
    pub fn shockwave_radius(self) -> f32 {
        match self {
            HostSneakAttackKind::GLASneakAttack => SNEAK_ATTACK_SHOCKWAVE_RADIUS,
        }
    }

    pub fn activate_audio(self) -> &'static str {
        match self {
            HostSneakAttackKind::GLASneakAttack => SNEAK_ATTACK_ACTIVATE_AUDIO,
        }
    }

    pub fn spawn_audio(self) -> &'static str {
        match self {
            HostSneakAttackKind::GLASneakAttack => SNEAK_ATTACK_SPAWN_AUDIO,
        }
    }
}

/// Lifecycle of a queued host sneak attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostSneakAttackPhase {
    /// Queued after DoSpecialPower; waiting for tunnel spawn frame.
    Queued,
    /// Tunnel structure created (and residual shockwave applied).
    Completed,
    /// Cancelled (source died / invalid) before spawn.
    Cancelled,
}

/// One pending or completed host sneak-attack mission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostSneakAttackMission {
    pub id: u32,
    pub kind: HostSneakAttackKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub activate_frame: u32,
    pub spawn_frame: u32,
    pub phase: HostSneakAttackPhase,
    /// Template used (or intended) for the tunnel structure.
    pub tunnel_template: String,
    /// Object id of tunnel successfully created at spawn (if any).
    pub spawned_tunnel_id: Option<ObjectId>,
    /// Units hit by residual shockwave at spawn.
    pub shockwave_hits: u32,
    /// Total residual shockwave damage applied at spawn.
    pub shockwave_damage_total: f32,
}

/// Spawn plan for one due sneak attack (computed before mutable create/damage).
#[derive(Debug, Clone)]
pub struct HostSneakAttackSpawnPlan {
    pub mission_id: u32,
    pub kind: HostSneakAttackKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub tunnel_template: String,
    pub shockwave_damage: f32,
    pub shockwave_radius: f32,
}

/// 2D distance check residual (host gameplay x/z plane).
pub fn in_sneak_shockwave_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

/// Whether residual target can take sneak-attack shockwave damage.
///
/// Retail RadiusDamageAffects = ALLIES ENEMIES NEUTRALS (hits everyone in radius
/// including allies). Residual still skips dead / under-construction structures
/// that are not fully built? Fail-closed: damage all alive non-under-construction.
pub fn is_legal_sneak_shockwave_target(is_alive: bool, under_construction: bool) -> bool {
    is_alive && !under_construction
}

/// Host residual registry for SneakAttack special power missions.
#[derive(Debug, Clone, Default)]
pub struct HostSneakAttackRegistry {
    next_id: u32,
    missions: HashMap<u32, HostSneakAttackMission>,
    completed_this_frame: Vec<u32>,
    activated_this_frame: Vec<u32>,
    /// Total activations (honesty).
    pub activation_count: u32,
    /// Total tunnels successfully spawned.
    pub tunnel_spawn_count: u32,
    /// Total residual shockwave hits across all spawns.
    pub shockwave_hit_count: u32,
}

impl HostSneakAttackRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            missions: HashMap::new(),
            completed_this_frame: Vec::new(),
            activated_this_frame: Vec::new(),
            activation_count: 0,
            tunnel_spawn_count: 0,
            shockwave_hit_count: 0,
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn clear_frame_events(&mut self) {
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
    }

    pub fn activation_count(&self) -> u32 {
        self.activation_count
    }

    pub fn tunnel_spawn_count(&self) -> u32 {
        self.tunnel_spawn_count
    }

    pub fn shockwave_hit_count(&self) -> u32 {
        self.shockwave_hit_count
    }

    pub fn mission_count(&self) -> usize {
        self.missions.len()
    }

    pub fn pending_count(&self) -> usize {
        self.missions
            .values()
            .filter(|m| m.phase == HostSneakAttackPhase::Queued)
            .count()
    }

    pub fn get(&self, id: u32) -> Option<&HostSneakAttackMission> {
        self.missions.get(&id)
    }

    pub fn missions_snapshot(&self) -> Vec<HostSneakAttackMission> {
        let mut v: Vec<_> = self.missions.values().cloned().collect();
        v.sort_by_key(|m| m.id);
        v
    }

    pub fn pending_of_kind(&self, kind: HostSneakAttackKind) -> Vec<&HostSneakAttackMission> {
        self.missions
            .values()
            .filter(|m| m.phase == HostSneakAttackPhase::Queued && m.kind == kind)
            .collect()
    }

    pub fn completed_of_kind(&self, kind: HostSneakAttackKind) -> Vec<&HostSneakAttackMission> {
        self.missions
            .values()
            .filter(|m| m.phase == HostSneakAttackPhase::Completed && m.kind == kind)
            .collect()
    }

    /// Queue a sneak-attack mission. Returns host mission id.
    pub fn queue(
        &mut self,
        kind: HostSneakAttackKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
        tunnel_template: impl Into<String>,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        let spawn_frame = activate_frame.saturating_add(kind.spawn_delay_frames());
        let mission = HostSneakAttackMission {
            id,
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            spawn_frame,
            phase: HostSneakAttackPhase::Queued,
            tunnel_template: tunnel_template.into(),
            spawned_tunnel_id: None,
            shockwave_hits: 0,
            shockwave_damage_total: 0.0,
        };
        self.missions.insert(id, mission);
        self.activated_this_frame.push(id);
        self.activation_count = self.activation_count.saturating_add(1);
        id
    }

    /// Build spawn plans for all missions whose spawn frame has arrived.
    pub fn plan_due_spawns(&self, current_frame: u32) -> Vec<HostSneakAttackSpawnPlan> {
        let mut plans = Vec::new();
        for mission in self.missions.values() {
            if mission.phase != HostSneakAttackPhase::Queued || current_frame < mission.spawn_frame {
                continue;
            }
            plans.push(HostSneakAttackSpawnPlan {
                mission_id: mission.id,
                kind: mission.kind,
                source_object: mission.source_object,
                source_team: mission.source_team,
                target_position: mission.target_position,
                tunnel_template: mission.tunnel_template.clone(),
                shockwave_damage: mission.kind.shockwave_damage(),
                shockwave_radius: mission.kind.shockwave_radius(),
            });
        }
        plans.sort_by_key(|p| p.mission_id);
        plans
    }

    /// Record spawn results after GameLogic created tunnel + applied shockwave.
    pub fn record_spawn_complete(
        &mut self,
        mission_id: u32,
        spawned_tunnel_id: Option<ObjectId>,
        shockwave_hits: u32,
        shockwave_damage_total: f32,
    ) {
        if let Some(mission) = self.missions.get_mut(&mission_id) {
            if mission.phase == HostSneakAttackPhase::Queued {
                mission.phase = HostSneakAttackPhase::Completed;
                mission.spawned_tunnel_id = spawned_tunnel_id;
                mission.shockwave_hits = shockwave_hits;
                mission.shockwave_damage_total = shockwave_damage_total;
                if spawned_tunnel_id.is_some() {
                    self.tunnel_spawn_count = self.tunnel_spawn_count.saturating_add(1);
                }
                self.shockwave_hit_count = self.shockwave_hit_count.saturating_add(shockwave_hits);
                self.completed_this_frame.push(mission_id);
            }
        }
    }

    /// Cancel pending missions owned by a destroyed source object.
    pub fn cancel_for_source(&mut self, source: ObjectId) {
        for mission in self.missions.values_mut() {
            if mission.source_object == source && mission.phase == HostSneakAttackPhase::Queued {
                mission.phase = HostSneakAttackPhase::Cancelled;
            }
        }
    }

    // --- Honesty flags (host residual; do not claim full retail parity) ---

    /// Residual honesty: at least one sneak attack activated/queued.
    pub fn honesty_activate_ok(&self) -> bool {
        self.activation_count > 0
    }

    /// Residual honesty: at least one tunnel structure spawned.
    pub fn honesty_tunnel_spawn_ok(&self) -> bool {
        self.tunnel_spawn_count > 0
    }

    /// Residual honesty: at least one unit hit by residual shockwave.
    pub fn honesty_shockwave_ok(&self) -> bool {
        self.shockwave_hit_count > 0
    }

    /// Combined host path: activated and spawned a tunnel.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_tunnel_spawn_ok()
    }
}


// --- Wave 62 residual honesty packs ---

/// Special-power residual honesty (ReloadTime / science / audio / radius).
pub fn honesty_sneak_attack_special_power_residual_ok() -> bool {
    SNEAK_ATTACK_SPECIAL_POWER_TEMPLATE == "SuperweaponSneakAttack"
        && SNEAK_ATTACK_REQUIRED_SCIENCE == "SCIENCE_SneakAttack"
        && SNEAK_ATTACK_RELOAD_TIME_MS == 150_000
        && SNEAK_ATTACK_RELOAD_TIME_FRAMES == 4500
        && (HOST_SNEAK_ATTACK_RADIUS - 50.0).abs() < 0.01
        && SNEAK_ATTACK_SHARED_SYNCED_TIMER
        && SNEAK_ATTACK_SHORTCUT_POWER
        && !SNEAK_ATTACK_PUBLIC_TIMER
        && SNEAK_ATTACK_ACTIVATE_AUDIO == "SneakAttackActivated"
        && HostSneakAttackKind::GLASneakAttack.activate_audio() == SNEAK_ATTACK_ACTIVATE_AUDIO
}

/// Tunnel residual honesty (template / health / vision / OCL / spawn delay).
pub fn honesty_sneak_attack_tunnel_residual_ok() -> bool {
    GLA_SNEAK_TUNNEL_TEMPLATE == "GLASneakAttackTunnelNetwork"
        && SNEAK_ATTACK_TUNNEL_START_TEMPLATE == "GLASneakAttackTunnelNetworkStart"
        && SNEAK_ATTACK_OCL_START == "OCL_CreateSneakAttackTunnelStart"
        && SNEAK_ATTACK_OCL_TUNNEL == "OCL_CreateSneakAttackTunnel"
        && (SNEAK_ATTACK_TUNNEL_MAX_HEALTH - 1000.0).abs() < 0.01
        && (SNEAK_ATTACK_TUNNEL_VISION_RANGE - 200.0).abs() < 0.01
        && SNEAK_ATTACK_LIFETIME_MS == 5_000
        && SNEAK_ATTACK_SPAWN_DELAY_FRAMES == 150
        && HostSneakAttackKind::GLASneakAttack.spawn_delay_frames() == 150
        && HostSneakAttackKind::GLASneakAttack.tunnel_template() == GLA_SNEAK_TUNNEL_TEMPLATE
}

/// Multi-shockwave spawn residual honesty matrix.
pub fn honesty_sneak_attack_spawn_residual_ok() -> bool {
    let pulses = sneak_attack_shockwave_pulses();
    pulses.len() == 3
        && pulses[0].weapon_name == "SneakAttackShockwaveWeaponSmall"
        && (pulses[0].primary_damage - 10.0).abs() < 0.01
        && (pulses[0].primary_radius - 35.0).abs() < 0.01
        && pulses[0].initial_delay_ms == 10
        && pulses[0].initial_delay_frames == 1
        && pulses[1].weapon_name == "SneakAttackShockwaveWeaponBig"
        && (pulses[1].primary_damage - 50.0).abs() < 0.01
        && (pulses[1].primary_radius - 50.0).abs() < 0.01
        && pulses[1].initial_delay_ms == 1000
        && pulses[1].initial_delay_frames == 30
        && pulses[2].initial_delay_ms == 2500
        && pulses[2].initial_delay_frames == 75
        && sneak_shockwave_pulse_frame(0, &pulses[0]) == 1
        && sneak_shockwave_pulse_frame(0, &pulses[1]) == 30
        && sneak_shockwave_pulse_frame(0, &pulses[2]) == 75
        // Host Big-only residual still matches second/third pulse damage.
        && (SNEAK_ATTACK_SHOCKWAVE_DAMAGE - 50.0).abs() < 0.01
        && (SNEAK_ATTACK_SHOCKWAVE_RADIUS - 50.0).abs() < 0.01
        && (SNEAK_SHOCKWAVE_MEDIUM_DAMAGE - 30.0).abs() < 0.01
        && (SNEAK_SHOCKWAVE_MEDIUM_RADIUS - 50.0).abs() < 0.01
}

/// Combined Wave 62 sneak-attack residual honesty pack.
pub fn honesty_sneak_attack_residual_pack_ok() -> bool {
    honesty_sneak_attack_special_power_residual_ok()
        && honesty_sneak_attack_tunnel_residual_ok()
        && honesty_sneak_attack_spawn_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn sneak_constants_match_retail_residual() {
        assert!((HOST_SNEAK_ATTACK_RADIUS - 50.0).abs() < 0.01);
        assert_eq!(SNEAK_ATTACK_SPAWN_DELAY_FRAMES, 150);
        assert!((SNEAK_ATTACK_SHOCKWAVE_DAMAGE - 50.0).abs() < 0.01);
        assert!((SNEAK_ATTACK_SHOCKWAVE_RADIUS - 50.0).abs() < 0.01);
        assert!(!SNEAK_ATTACK_ACTIVATE_AUDIO.is_empty());
    }

    #[test]
    fn sneak_maps_from_command_power() {
        assert_eq!(
            HostSneakAttackKind::from_command_power(&SpecialPowerType::SneakAttack),
            Some(HostSneakAttackKind::GLASneakAttack)
        );
        assert_eq!(
            HostSneakAttackKind::from_command_power(&SpecialPowerType::Ambush),
            None
        );
        assert_eq!(
            HostSneakAttackKind::from_command_power(&SpecialPowerType::Paradrop),
            None
        );
    }

    #[test]
    fn queue_and_complete_sneak_spawn_plan() {
        let mut reg = HostSneakAttackRegistry::new();
        let id = reg.queue(
            HostSneakAttackKind::GLASneakAttack,
            ObjectId(1),
            Team::GLA,
            Vec3::new(100.0, 0.0, 50.0),
            0,
            SNEAK_ATTACK_RESIDUAL_TEMPLATE,
        );
        assert!(reg.honesty_activate_ok());
        assert!(!reg.honesty_tunnel_spawn_ok());
        assert_eq!(reg.pending_count(), 1);
        assert_eq!(
            reg.get(id).unwrap().spawn_frame,
            SNEAK_ATTACK_SPAWN_DELAY_FRAMES
        );

        assert!(reg.plan_due_spawns(SNEAK_ATTACK_SPAWN_DELAY_FRAMES - 1).is_empty());
        let plans = reg.plan_due_spawns(SNEAK_ATTACK_SPAWN_DELAY_FRAMES);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].tunnel_template, SNEAK_ATTACK_RESIDUAL_TEMPLATE);

        reg.record_spawn_complete(id, Some(ObjectId(42)), 2, 100.0);
        assert!(reg.honesty_tunnel_spawn_ok());
        assert!(reg.honesty_shockwave_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.tunnel_spawn_count(), 1);
        assert_eq!(reg.shockwave_hit_count(), 2);
        assert_eq!(reg.pending_count(), 0);
    }

    #[test]
    fn shockwave_radius_and_target_filter() {
        assert!(in_sneak_shockwave_radius_2d((0.0, 0.0), (50.0, 0.0), 50.0));
        assert!(!in_sneak_shockwave_radius_2d((0.0, 0.0), (51.0, 0.0), 50.0));
        assert!(is_legal_sneak_shockwave_target(true, false));
        assert!(!is_legal_sneak_shockwave_target(false, false));
        assert!(!is_legal_sneak_shockwave_target(true, true));
    }

    #[test]
    fn sneak_attack_residual_pack_honesty() {
        assert!(honesty_sneak_attack_special_power_residual_ok());
        assert!(honesty_sneak_attack_tunnel_residual_ok());
        assert!(honesty_sneak_attack_spawn_residual_ok());
        assert!(honesty_sneak_attack_residual_pack_ok());
    }
}
