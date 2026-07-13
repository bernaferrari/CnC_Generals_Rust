//! Host America Aurora aircraft bomb residual (dive + FuelAir area damage).
//!
//! Residual slice (playability):
//! - AmericaJetAurora (and AirF / SupW / Lazr variants) attack queues a delayed
//!   dive-bomb residual at the target position (retail AuroraBombWeapon /
//!   SupW_AuroraFuelBombWeapon → AuroraBomb / SupW_AuroraFuelAirBomb projectile
//!   with MissileAIUpdate dive + HeightDieUpdate).
//! - After dive (+ FuelAir gas) delay frames, area damage is applied at the
//!   impact epicenter (retail AuroraBombWeapon PrimaryDamage 400 / r20, or
//!   AirF_AuroraBombDetonationWeapon / SupW_FuelBombDetonationWeapon for FuelAir).
//! - RadiusDamageAffects ALLIES residual: blast hits same-team units (source
//!   aircraft Object still excluded). Retail AuroraBombWeapon /
//!   AirF_AuroraBombDetonationWeapon list ALLIES ENEMIES NEUTRALS.
//! - FuelAir DaisyCutterFlameWeapon secondary residual (AirF_AuroraBombGas /
//!   SupW_AuroraFuelAirGas SlowDeath MIDPOINT — tree-ignite flame 5 / r100).
//! - Honesty counters for activate / complete / damage gates and tests.
//!
//! Fail-closed honesty:
//! - Not full AuroraBombLocomotor / MissileAIUpdate DistanceToTargetBeforeDiving
//! - Not full HeightDieUpdate / CreateObjectDie OCL_AuroraBombExplode gas object
//! - Not full SlowDeath multi-stage timing / tree burn state / FX GPU
//! - Not full JetAIUpdate SET_SUPERSONIC sneak offset / RETURN_TO_BASE clip reload
//! - Not full SupW_FuelBombDetonationWeapon 900/r70 matrix (FuelAir residual uses
//!   AirF 1000/r100 host numbers for AirF + SupW collapse)
//! - Not multiplayer shared-synced bomb projectile (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const AURORA_LOGIC_FPS: f32 = 30.0;

// --- Standard AuroraBombWeapon residual (Weapon.ini) ---
/// Retail AuroraBombWeapon PrimaryDamage.
pub const AURORA_BOMB_DAMAGE: f32 = 400.0;
/// Retail AuroraBombWeapon PrimaryDamageRadius.
pub const AURORA_BOMB_RADIUS: f32 = 20.0;
/// Residual bomb dive / fall delay before impact (1.5 s @ 30 FPS).
/// Fail-closed vs full AuroraBombLocomotor flight time.
pub const AURORA_BOMB_DIVE_DELAY_FRAMES: u32 = 45;

// --- Fuel-Air Aurora residual (AirF_AuroraBomb / SupW_AuroraFuelAirBomb) ---
/// Retail AirF_AuroraBombDetonationWeapon PrimaryDamage (FuelAir FINAL weapon).
pub const AURORA_FUEL_AIR_DAMAGE: f32 = 1000.0;
/// Retail AirF_AuroraBombDetonationWeapon PrimaryDamageRadius.
pub const AURORA_FUEL_AIR_RADIUS: f32 = 100.0;
/// SupW_FuelBombDetonationWeapon PrimaryDamage residual (secondary FuelAir path).
pub const AURORA_FUEL_AIR_SUPW_DAMAGE: f32 = 900.0;
/// SupW_FuelBombDetonationWeapon PrimaryDamageRadius.
pub const AURORA_FUEL_AIR_SUPW_RADIUS: f32 = 70.0;
/// Retail AuroraBombGas / SupW_AuroraFuelAirGas DestructionDelay = 1000 ms → 30 frames.
pub const AURORA_FUEL_AIR_GAS_DELAY_FRAMES: u32 = 30;
/// Combined dive + gas detonation delay for FuelAir residual.
pub const AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES: u32 =
    AURORA_BOMB_DIVE_DELAY_FRAMES + AURORA_FUEL_AIR_GAS_DELAY_FRAMES;

// --- DaisyCutterFlameWeapon secondary residual (FuelAir gas SlowDeath MIDPOINT) ---
/// Retail `DaisyCutterFlameWeapon` PrimaryDamage (spot of flame to light trees).
pub const AURORA_FUEL_AIR_FLAME_DAMAGE: f32 = 5.0;
/// Retail `DaisyCutterFlameWeapon` PrimaryDamageRadius.
pub const AURORA_FUEL_AIR_FLAME_RADIUS: f32 = 100.0;
/// Residual flame ignite cue (AirF_FX_AuroraBombIgnite / DaisyCutterIgnite family).
pub const AURORA_FUEL_AIR_FLAME_AUDIO: &str = "DaisyCutterIgnite";

/// Retail AuroraBombWeapon AttackRange residual.
pub const AURORA_BOMB_ATTACK_RANGE: f32 = 300.0;

/// Retail primary weapon template name (standard Aurora).
pub const AURORA_BOMB_PRIMARY_WEAPON: &str = "AuroraBombWeapon";
/// Retail AirF primary weapon (tiny impact; detonation via OCL residual).
pub const AIRF_AURORA_BOMB_PRIMARY_WEAPON: &str = "AirF_AuroraBombWeapon";
/// Retail SupW Fuel-Air Aurora primary.
pub const SUPW_AURORA_FUEL_BOMB_WEAPON: &str = "SupW_AuroraFuelBombWeapon";

/// Activate / drop audio residual (FX_AuroraBombLaunch / Weapon fire cue).
pub const AURORA_BOMB_LAUNCH_AUDIO: &str = "AuroraBombLaunch";
/// Impact / detonation audio residual (FX_AuroraBombDetonate / FuelAir).
pub const AURORA_BOMB_DETONATE_AUDIO: &str = "AuroraBombDetonate";
/// Fuel-Air detonation audio residual (DaisyCutter-family final explosion cue).
pub const AURORA_FUEL_AIR_DETONATE_AUDIO: &str = "DaisyCutterExplosion";

/// Host residual Aurora bomb kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostAuroraBombKind {
    /// Standard AmericaJetAurora AuroraBombWeapon residual (400 dmg / r20).
    Standard,
    /// AirF / SupW Fuel-Air Aurora residual (delayed gas detonation area damage).
    FuelAir,
}

impl HostAuroraBombKind {
    pub fn label(self) -> &'static str {
        match self {
            HostAuroraBombKind::Standard => "AuroraBomb",
            HostAuroraBombKind::FuelAir => "AuroraFuelAir",
        }
    }

    /// Absolute delay frames from activate → area damage.
    pub fn impact_delay_frames(self) -> u32 {
        match self {
            HostAuroraBombKind::Standard => AURORA_BOMB_DIVE_DELAY_FRAMES,
            HostAuroraBombKind::FuelAir => AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES,
        }
    }

    /// Primary residual blast damage.
    pub fn damage(self) -> f32 {
        match self {
            HostAuroraBombKind::Standard => AURORA_BOMB_DAMAGE,
            HostAuroraBombKind::FuelAir => AURORA_FUEL_AIR_DAMAGE,
        }
    }

    /// Primary residual blast radius.
    pub fn radius(self) -> f32 {
        match self {
            HostAuroraBombKind::Standard => AURORA_BOMB_RADIUS,
            HostAuroraBombKind::FuelAir => AURORA_FUEL_AIR_RADIUS,
        }
    }

    /// Inner radius with full damage (two-stage falloff residual).
    pub fn falloff_inner(self) -> f32 {
        match self {
            HostAuroraBombKind::Standard => AURORA_BOMB_RADIUS * 0.5,
            HostAuroraBombKind::FuelAir => AURORA_FUEL_AIR_RADIUS * 0.5,
        }
    }

    pub fn activate_audio(self) -> &'static str {
        AURORA_BOMB_LAUNCH_AUDIO
    }

    pub fn impact_audio(self) -> &'static str {
        match self {
            HostAuroraBombKind::Standard => AURORA_BOMB_DETONATE_AUDIO,
            HostAuroraBombKind::FuelAir => AURORA_FUEL_AIR_DETONATE_AUDIO,
        }
    }

    /// Whether impact also applies retail `DaisyCutterFlameWeapon` secondary residual
    /// (AirF_AuroraBombGas / SupW_AuroraFuelAirGas SlowDeath MIDPOINT flame).
    pub fn spawns_daisy_cutter_flame(self) -> bool {
        matches!(self, HostAuroraBombKind::FuelAir)
    }
}

/// Lifecycle of a queued host Aurora bomb dive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostAuroraBombPhase {
    /// Queued after attack fire; bomb diving / gas igniting.
    Queued,
    /// Area damage resolved at target.
    Completed,
    /// Cancelled before impact (fail-closed residual).
    Cancelled,
}

/// One pending or completed host Aurora dive-bomb mission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostAuroraBombMission {
    pub id: u32,
    pub kind: HostAuroraBombKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub activate_frame: u32,
    pub impact_frame: u32,
    pub phase: HostAuroraBombPhase,
    /// Total residual HP damage dealt this impact.
    pub damage_dealt: f32,
    /// Objects hit by residual blast.
    pub objects_hit: u32,
    /// Objects destroyed by residual blast.
    pub objects_destroyed: u32,
}

/// Damage plan for one due Aurora bomb (computed before mutable damage).
#[derive(Debug, Clone)]
pub struct HostAuroraBombImpactPlan {
    pub mission_id: u32,
    pub kind: HostAuroraBombKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub hits: Vec<HostAuroraBombHit>,
}

/// One victim planned for residual blast damage.
#[derive(Debug, Clone, Copy)]
pub struct HostAuroraBombHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub distance: f32,
}

/// Whether template is a residual Aurora aircraft that drops dive bombs.
///
/// Fail-closed: name residual (not full JetAIUpdate / WeaponSet matrix).
/// Excludes projectile / bomb / gas objects (`AuroraBomb`, `AirF_AuroraBombGas`).
pub fn is_aurora_aircraft_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n == "testaurora" || n == "test_aurora" || n == "testaurorafuelair" {
        return true;
    }
    // Projectile / gas / detonation objects are not the plane.
    if n.contains("bomb")
        || n.contains("gas")
        || n.contains("projectile")
        || n.contains("detonation")
        || n.contains("weapon")
        || n.contains("locomotor")
    {
        return false;
    }
    // Canonical + general variants: AmericaJetAurora, AirF_AmericaJetAurora,
    // SupW_AmericaJetAurora, Lazr_AmericaJetAurora, USA_Aurora residual aliases.
    n.contains("jetaurora")
        || (n.contains("aurora")
            && (n.contains("jet") || n.contains("bomber") || n.starts_with("usa")))
}

/// Classify residual bomb kind from Aurora aircraft template name.
///
/// Airforce / Superweapon General Auroras use FuelAir residual path.
/// Standard AmericaJetAurora uses AuroraBombWeapon residual.
pub fn aurora_bomb_kind_for_template(template_name: &str) -> HostAuroraBombKind {
    let n = template_name.to_ascii_lowercase();
    if n == "testaurorafuelair"
        || n.starts_with("airf")
        || n.starts_with("supw")
        || n.contains("fuelair")
        || n.contains("fuel_air")
    {
        return HostAuroraBombKind::FuelAir;
    }
    HostAuroraBombKind::Standard
}

/// Residual two-stage falloff: full damage inside half-radius, linear to edge.
pub fn aurora_bomb_damage_at_distance(kind: HostAuroraBombKind, distance: f32) -> f32 {
    let radius = kind.radius();
    if distance > radius || radius <= 0.0 {
        return 0.0;
    }
    let base = kind.damage();
    let inner = kind.falloff_inner();
    if distance <= inner {
        return base;
    }
    let t = (distance - inner) / (radius - inner).max(0.001);
    base * (1.0 - t).max(0.0)
}

/// Whether residual target receives Aurora bomb area damage.
///
/// Retail `RadiusDamageAffects = ALLIES ENEMIES NEUTRALS` (Standard also lists
/// NOT_SIMILAR). Host residual hits living non-self units of any team — source
/// aircraft Object still excluded. Fail-closed: not full NOT_SIMILAR / Relationship
/// matrix beyond team residual.
pub fn is_legal_aurora_bomb_target(is_alive: bool, is_self: bool) -> bool {
    is_alive && !is_self
}

/// 2D distance residual (host gameplay x/z plane).
pub fn distance_2d(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

/// Host residual registry for Aurora dive-bomb missions.
#[derive(Debug, Clone, Default)]
pub struct HostAuroraBombRegistry {
    next_id: u32,
    missions: HashMap<u32, HostAuroraBombMission>,
    completed_this_frame: Vec<u32>,
    activated_this_frame: Vec<u32>,
    /// Total activations / queued dive bombs (honesty).
    pub activation_count: u32,
    /// Total completed detonations (honesty).
    pub completion_count: u32,
    /// Total residual HP damage dealt across all impacts (honesty).
    pub damage_dealt: f32,
    /// Total objects destroyed by residual blasts.
    pub objects_destroyed: u32,
}

impl HostAuroraBombRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            missions: HashMap::new(),
            completed_this_frame: Vec::new(),
            activated_this_frame: Vec::new(),
            activation_count: 0,
            completion_count: 0,
            damage_dealt: 0.0,
            objects_destroyed: 0,
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

    pub fn completion_count(&self) -> u32 {
        self.completion_count
    }

    pub fn damage_dealt(&self) -> f32 {
        self.damage_dealt
    }

    pub fn mission_count(&self) -> usize {
        self.missions.len()
    }

    pub fn pending_count(&self) -> usize {
        self.missions
            .values()
            .filter(|m| m.phase == HostAuroraBombPhase::Queued)
            .count()
    }

    pub fn get(&self, id: u32) -> Option<&HostAuroraBombMission> {
        self.missions.get(&id)
    }

    pub fn missions_snapshot(&self) -> Vec<HostAuroraBombMission> {
        let mut v: Vec<_> = self.missions.values().cloned().collect();
        v.sort_by_key(|m| m.id);
        v
    }

    pub fn pending_of_kind(&self, kind: HostAuroraBombKind) -> Vec<&HostAuroraBombMission> {
        self.missions
            .values()
            .filter(|m| m.phase == HostAuroraBombPhase::Queued && m.kind == kind)
            .collect()
    }

    pub fn completed_of_kind(&self, kind: HostAuroraBombKind) -> Vec<&HostAuroraBombMission> {
        self.missions
            .values()
            .filter(|m| m.phase == HostAuroraBombPhase::Completed && m.kind == kind)
            .collect()
    }

    /// Queue a delayed Aurora dive bomb. Returns host mission id.
    pub fn queue(
        &mut self,
        kind: HostAuroraBombKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        let impact_frame = activate_frame.saturating_add(kind.impact_delay_frames());
        let mission = HostAuroraBombMission {
            id,
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            impact_frame,
            phase: HostAuroraBombPhase::Queued,
            damage_dealt: 0.0,
            objects_hit: 0,
            objects_destroyed: 0,
        };
        self.missions.insert(id, mission);
        self.activated_this_frame.push(id);
        self.activation_count = self.activation_count.saturating_add(1);
        id
    }

    /// Build impact plans for all missions whose dive/gas delay has arrived.
    ///
    /// `objects`: (id, position, team, is_alive)
    pub fn plan_due_impacts(
        &self,
        current_frame: u32,
        objects: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostAuroraBombImpactPlan> {
        let mut plans = Vec::new();
        for mission in self.missions.values() {
            if mission.phase != HostAuroraBombPhase::Queued || current_frame < mission.impact_frame {
                continue;
            }
            let mut hits = Vec::new();
            for &(id, pos, _team, alive) in objects {
                let is_self = id == mission.source_object;
                if !is_legal_aurora_bomb_target(alive, is_self) {
                    continue;
                }
                let dist = distance_2d(mission.target_position, pos);
                let primary = aurora_bomb_damage_at_distance(mission.kind, dist);
                // DaisyCutterFlameWeapon secondary residual (FuelAir gas MIDPOINT).
                // Fail-closed: not full SlowDeath MIDPOINT timing / tree burn state.
                let flame = if mission.kind.spawns_daisy_cutter_flame()
                    && dist <= AURORA_FUEL_AIR_FLAME_RADIUS
                {
                    AURORA_FUEL_AIR_FLAME_DAMAGE
                } else {
                    0.0
                };
                let dmg = primary + flame;
                if dmg > 0.0 {
                    hits.push(HostAuroraBombHit {
                        target_id: id,
                        damage: dmg,
                        distance: dist,
                    });
                }
            }
            hits.sort_by_key(|h| h.target_id.0);
            plans.push(HostAuroraBombImpactPlan {
                mission_id: mission.id,
                kind: mission.kind,
                source_object: mission.source_object,
                source_team: mission.source_team,
                target_position: mission.target_position,
                hits,
            });
        }
        plans.sort_by_key(|p| p.mission_id);
        plans
    }

    /// Record impact results after GameLogic applied area damage.
    pub fn record_impact_complete(
        &mut self,
        mission_id: u32,
        damage_dealt: f32,
        objects_hit: u32,
        objects_destroyed: u32,
    ) {
        if let Some(mission) = self.missions.get_mut(&mission_id) {
            if mission.phase == HostAuroraBombPhase::Queued {
                mission.phase = HostAuroraBombPhase::Completed;
                mission.damage_dealt = damage_dealt.max(0.0);
                mission.objects_hit = objects_hit;
                mission.objects_destroyed = objects_destroyed;
                self.completion_count = self.completion_count.saturating_add(1);
                self.damage_dealt += damage_dealt.max(0.0);
                self.objects_destroyed = self
                    .objects_destroyed
                    .saturating_add(objects_destroyed);
                self.completed_this_frame.push(mission_id);
            }
        }
    }

    /// Cancel pending missions owned by a destroyed source (fail-closed residual).
    pub fn cancel_for_source(&mut self, source: ObjectId) {
        for mission in self.missions.values_mut() {
            if mission.source_object == source && mission.phase == HostAuroraBombPhase::Queued {
                mission.phase = HostAuroraBombPhase::Cancelled;
            }
        }
    }

    // --- Honesty flags (host residual; do not claim full retail parity) ---

    /// Residual honesty: at least one Aurora bomb dive activated/queued.
    pub fn honesty_activate_ok(&self) -> bool {
        self.activation_count > 0
    }

    /// Residual honesty: at least one delayed detonation completed.
    pub fn honesty_complete_ok(&self) -> bool {
        self.completion_count > 0
    }

    /// Residual honesty: at least some blast damage was dealt.
    pub fn honesty_damage_ok(&self) -> bool {
        self.damage_dealt > 0.0
    }

    /// Combined host path: activated + completed detonation + damage dealt.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_complete_ok() && self.honesty_damage_ok()
    }

    /// Kind-specific complete honesty (for FuelAir vs Standard tests).
    pub fn honesty_complete_ok_of_kind(&self, kind: HostAuroraBombKind) -> bool {
        !self.completed_of_kind(kind).is_empty()
    }

    /// Kind-specific queue honesty.
    pub fn honesty_queue_ok_of_kind(&self, kind: HostAuroraBombKind) -> bool {
        !self.pending_of_kind(kind).is_empty() || !self.completed_of_kind(kind).is_empty()
    }
}

/// Residual primary weapon binding for Aurora aircraft (host combat path).
///
/// ClipSize residual = 1 (RETURN_TO_BASE reload not modeled — long host reload).
pub fn aurora_bomb_weapon(kind: HostAuroraBombKind) -> super::Weapon {
    // Host residual: weapon.damage is residual bookkeeping only — area damage is
    // applied after dive delay, not as instant single-target take_damage.
    // Use small residual damage for any path that still reads weapon.damage.
    let bookkeeping_damage = match kind {
        HostAuroraBombKind::Standard => AURORA_BOMB_DAMAGE,
        // AirF primary is intentionally tiny (2.0); detonation is residual OCL.
        HostAuroraBombKind::FuelAir => 2.0,
    };
    super::Weapon {
        damage: bookkeeping_damage,
        range: AURORA_BOMB_ATTACK_RANGE,
        min_range: 0.0,
        reload_time: 5.0, // ClipReloadTime 5000 ms residual
        last_fire_time: -100.0,
        ammo: Some(1),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 99999.0,
        pre_attack_delay: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn aurora_constants_match_retail_residual() {
        assert!((AURORA_BOMB_DAMAGE - 400.0).abs() < 0.01);
        assert!((AURORA_BOMB_RADIUS - 20.0).abs() < 0.01);
        assert_eq!(AURORA_BOMB_DIVE_DELAY_FRAMES, 45);
        assert!((AURORA_FUEL_AIR_DAMAGE - 1000.0).abs() < 0.01);
        assert!((AURORA_FUEL_AIR_RADIUS - 100.0).abs() < 0.01);
        assert_eq!(AURORA_FUEL_AIR_GAS_DELAY_FRAMES, 30);
        assert_eq!(AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES, 75);
        assert!((AURORA_BOMB_ATTACK_RANGE - 300.0).abs() < 0.1);
        // DaisyCutterFlameWeapon secondary residual (FuelAir gas MIDPOINT).
        assert!((AURORA_FUEL_AIR_FLAME_DAMAGE - 5.0).abs() < 0.01);
        assert!((AURORA_FUEL_AIR_FLAME_RADIUS - 100.0).abs() < 0.1);
        assert!(HostAuroraBombKind::FuelAir.spawns_daisy_cutter_flame());
        assert!(!HostAuroraBombKind::Standard.spawns_daisy_cutter_flame());
    }

    #[test]
    fn allies_residual_and_legal_target_honesty() {
        // Alive non-self is legal (ALLIES residual); dead / self excluded.
        assert!(is_legal_aurora_bomb_target(true, false));
        assert!(!is_legal_aurora_bomb_target(false, false));
        assert!(!is_legal_aurora_bomb_target(true, true));
        assert!(!is_legal_aurora_bomb_target(false, true));
    }

    #[test]
    fn aurora_aircraft_name_matrix() {
        assert!(is_aurora_aircraft_template("AmericaJetAurora"));
        assert!(is_aurora_aircraft_template("AirF_AmericaJetAurora"));
        assert!(is_aurora_aircraft_template("SupW_AmericaJetAurora"));
        assert!(is_aurora_aircraft_template("Lazr_AmericaJetAurora"));
        assert!(is_aurora_aircraft_template("TestAurora"));
        assert!(is_aurora_aircraft_template("TestAuroraFuelAir"));
        assert!(is_aurora_aircraft_template("USA_Aurora"));
        // Projectiles / gas are not the plane.
        assert!(!is_aurora_aircraft_template("AuroraBomb"));
        assert!(!is_aurora_aircraft_template("AirF_AuroraBomb"));
        assert!(!is_aurora_aircraft_template("AirF_AuroraBombGas"));
        assert!(!is_aurora_aircraft_template("SupW_AuroraFuelAirBomb"));
        assert!(!is_aurora_aircraft_template("AuroraBombWeapon"));
        assert!(!is_aurora_aircraft_template("USA_Ranger"));
        assert!(!is_aurora_aircraft_template("AmericaJetStealthFighter"));
    }

    #[test]
    fn fuel_air_kind_from_template() {
        assert_eq!(
            aurora_bomb_kind_for_template("AmericaJetAurora"),
            HostAuroraBombKind::Standard
        );
        assert_eq!(
            aurora_bomb_kind_for_template("TestAurora"),
            HostAuroraBombKind::Standard
        );
        assert_eq!(
            aurora_bomb_kind_for_template("AirF_AmericaJetAurora"),
            HostAuroraBombKind::FuelAir
        );
        assert_eq!(
            aurora_bomb_kind_for_template("SupW_AmericaJetAurora"),
            HostAuroraBombKind::FuelAir
        );
        assert_eq!(
            aurora_bomb_kind_for_template("TestAuroraFuelAir"),
            HostAuroraBombKind::FuelAir
        );
    }

    #[test]
    fn damage_falloff_full_then_zero() {
        let kind = HostAuroraBombKind::Standard;
        assert!((aurora_bomb_damage_at_distance(kind, 0.0) - 400.0).abs() < 0.01);
        assert!((aurora_bomb_damage_at_distance(kind, 5.0) - 400.0).abs() < 0.01);
        assert_eq!(aurora_bomb_damage_at_distance(kind, 21.0), 0.0);
        let mid = aurora_bomb_damage_at_distance(kind, 15.0);
        assert!(mid > 0.0 && mid < 400.0, "mid falloff expected, got {mid}");

        let fa = HostAuroraBombKind::FuelAir;
        assert!((aurora_bomb_damage_at_distance(fa, 0.0) - 1000.0).abs() < 0.01);
        assert_eq!(aurora_bomb_damage_at_distance(fa, 101.0), 0.0);
    }

    #[test]
    fn queue_and_complete_delayed_dive_plan() {
        let mut reg = HostAuroraBombRegistry::new();
        let id = reg.queue(
            HostAuroraBombKind::FuelAir,
            ObjectId(1),
            Team::USA,
            Vec3::new(100.0, 0.0, 0.0),
            0,
        );
        assert!(reg.honesty_activate_ok());
        assert!(!reg.honesty_complete_ok());
        assert!(!reg.honesty_damage_ok());
        assert_eq!(reg.pending_count(), 1);
        assert_eq!(
            reg.get(id).unwrap().impact_frame,
            AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES
        );

        let objects = vec![
            (ObjectId(1), Vec3::new(0.0, 0.0, 0.0), Team::USA, true), // source — skip
            (ObjectId(2), Vec3::new(100.0, 0.0, 0.0), Team::GLA, true), // epicenter
            (ObjectId(3), Vec3::new(100.0, 0.0, 500.0), Team::GLA, true), // far
            (ObjectId(4), Vec3::new(100.0, 0.0, 0.0), Team::USA, true), // friend — ALLIES residual
        ];

        // Before delay: no plans.
        let early = reg.plan_due_impacts(AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES - 1, &objects);
        assert!(early.is_empty(), "no damage plan before dive delay");

        let plans = reg.plan_due_impacts(AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES, &objects);
        assert_eq!(plans.len(), 1);
        // Enemy + friend at epicenter; source excluded; far outside primary+flame.
        assert_eq!(plans[0].hits.len(), 2, "enemy + ally at epicenter (ALLIES residual)");
        let enemy = plans[0]
            .hits
            .iter()
            .find(|h| h.target_id == ObjectId(2))
            .expect("enemy hit");
        let ally = plans[0]
            .hits
            .iter()
            .find(|h| h.target_id == ObjectId(4))
            .expect("ally hit");
        // FuelAir primary 1000 + DaisyCutterFlame secondary 5 at epicenter.
        let expected = AURORA_FUEL_AIR_DAMAGE + AURORA_FUEL_AIR_FLAME_DAMAGE;
        assert!((enemy.damage - expected).abs() < 0.01);
        assert!((ally.damage - expected).abs() < 0.01);
        assert!(
            !plans[0].hits.iter().any(|h| h.target_id == ObjectId(1)),
            "source aircraft must be excluded"
        );
        assert!(
            !plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)),
            "far unit outside primary+flame radius must not be hit"
        );

        reg.record_impact_complete(id, expected * 2.0, 2, 0);
        assert!(reg.honesty_complete_ok());
        assert!(reg.honesty_damage_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.pending_count(), 0);
    }

    #[test]
    fn fuel_air_flame_and_allies_residual_honesty() {
        let mut reg = HostAuroraBombRegistry::new();
        let _id = reg.queue(
            HostAuroraBombKind::FuelAir,
            ObjectId(10),
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
            0,
        );
        // Ally at 80 (inside flame 100 + primary 100) vs ally at 120 (outside both).
        let objects = vec![
            (ObjectId(10), Vec3::new(500.0, 0.0, 0.0), Team::USA, true), // source
            (ObjectId(11), Vec3::new(80.0, 0.0, 0.0), Team::USA, true),  // ally mid
            (ObjectId(12), Vec3::new(120.0, 0.0, 0.0), Team::USA, true), // ally outer
            (ObjectId(13), Vec3::new(0.0, 0.0, 0.0), Team::GLA, true),   // enemy epicenter
        ];
        let plans = reg.plan_due_impacts(AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES, &objects);
        assert_eq!(plans.len(), 1);
        let hits = &plans[0].hits;
        let mid = hits.iter().find(|h| h.target_id == ObjectId(11)).expect("mid ally");
        let epic = hits.iter().find(|h| h.target_id == ObjectId(13)).expect("epic enemy");
        assert!(
            !hits.iter().any(|h| h.target_id == ObjectId(12)),
            "ally beyond primary+flame radius must not take residual damage"
        );
        // Mid at 80: primary falloff + full flame 5.
        assert!(mid.damage > AURORA_FUEL_AIR_FLAME_DAMAGE);
        assert!(mid.damage < AURORA_FUEL_AIR_DAMAGE + AURORA_FUEL_AIR_FLAME_DAMAGE);
        assert!(
            (epic.damage - (AURORA_FUEL_AIR_DAMAGE + AURORA_FUEL_AIR_FLAME_DAMAGE)).abs() < 0.1
        );

        // Standard Aurora: no flame secondary; still hits allies.
        let mut reg2 = HostAuroraBombRegistry::new();
        let _ = reg2.queue(
            HostAuroraBombKind::Standard,
            ObjectId(1),
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
            0,
        );
        let objects2 = vec![
            (ObjectId(1), Vec3::new(50.0, 0.0, 0.0), Team::USA, true),
            (ObjectId(2), Vec3::new(0.0, 0.0, 0.0), Team::USA, true), // ally epicenter
            (ObjectId(3), Vec3::new(0.0, 0.0, 0.0), Team::GLA, true),
        ];
        let plans2 = reg2.plan_due_impacts(AURORA_BOMB_DIVE_DELAY_FRAMES, &objects2);
        assert_eq!(plans2[0].hits.len(), 2);
        for h in &plans2[0].hits {
            assert!(
                (h.damage - AURORA_BOMB_DAMAGE).abs() < 0.01,
                "standard Aurora has no flame secondary"
            );
        }
    }

    #[test]
    fn residual_weapon_is_one_shot_ground() {
        let w = aurora_bomb_weapon(HostAuroraBombKind::Standard);
        assert!((w.range - AURORA_BOMB_ATTACK_RANGE).abs() < f32::EPSILON);
        assert!(w.can_target_ground);
        assert!(!w.can_target_air);
        assert_eq!(w.ammo, Some(1));
    }
}
