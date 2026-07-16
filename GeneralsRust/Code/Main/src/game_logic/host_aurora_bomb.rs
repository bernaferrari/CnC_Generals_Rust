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
//! Wave 61 residual pack (retail Weapon.ini / WeaponObjects.ini / AmericaAir.ini honesty):
//! - AuroraBombWeapon: Primary **400**/r**20**, AttackRange **300**, ClipSize **1**,
//!   ClipReload **5000**ms → **150**f, AutoReloadsClip **RETURN_TO_BASE**,
//!   DamageType **AURORA_BOMB**, DeathType **EXPLODED**, Projectile **AuroraBomb**,
//!   AcceptableAimDelta **45**, RadiusDamageAffects ALLIES ENEMIES NEUTRALS NOT_SIMILAR,
//!   ProjectileCollidesWith **STRUCTURES**, FireFX / DetonationFX residual names
//! - AirF detonation **1000**/r**100**, SupW detonation **900**/r**70**, primary tiny **2**/r**4**
//! - Projectile residual: MaxHealth **100**, Mass **75**, AuroraBombLocomotor Speed **480** /
//!   MinSpeed **240** / Accel **960** / TurnRate **960** / MaxThrustAngle **60**
//! - Jet body residual: MaxHealth **80**, Vision **180**, Shroud **600**, BuildCost **2500**,
//!   ReturnToBaseIdleTime **10000**ms → **300**f, SneakyOffsetWhenAttacking **-20**
//!
//! Fail-closed honesty:
//! - Not full AuroraBombLocomotor flight path / MissileAIUpdate DistanceToTargetBeforeDiving
//! - Not full HeightDieUpdate / CreateObjectDie OCL_AuroraBombExplode gas object
//! - Not full SlowDeath multi-stage timing / tree burn state / FX GPU
//! - Not full JetAIUpdate SET_SUPERSONIC sneak offset / airfield RETURN_TO_BASE rearm path
//!   (ClipSize/ClipReload/AutoReloadsClip residual honesty closed Wave 61)
//! - SupW_FuelBombDetonationWeapon 900/r70 residual is host-testable via
//!   `HostAuroraBombKind::FuelAirSupW` (AirF keeps 1000/r100)
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

// --- Wave 61 RETURN_TO_BASE / weapon matrix residual ---

/// Retail AuroraBombWeapon ClipSize residual.
pub const AURORA_BOMB_CLIP_SIZE: u32 = 1;
/// Retail AuroraBombWeapon ClipReloadTime residual (msec).
pub const AURORA_BOMB_CLIP_RELOAD_MS: u32 = 5000;
/// ClipReloadTime 5000ms → 150 frames @ 30 FPS.
pub const AURORA_BOMB_CLIP_RELOAD_FRAMES: u32 = 150;
/// Retail AutoReloadsClip residual.
pub const AURORA_BOMB_AUTO_RELOADS: &str = "RETURN_TO_BASE";
/// Retail DamageType residual.
pub const AURORA_BOMB_DAMAGE_TYPE: &str = "AURORA_BOMB";
/// Retail DeathType residual.
pub const AURORA_BOMB_DEATH_TYPE: &str = "EXPLODED";
/// Retail ProjectileObject residual (standard).
pub const AURORA_BOMB_PROJECTILE: &str = "AuroraBomb";
/// Retail AirF projectile residual.
pub const AIRF_AURORA_BOMB_PROJECTILE: &str = "AirF_AuroraBomb";
/// Retail SupW projectile residual.
pub const SUPW_AURORA_FUEL_AIR_BOMB_PROJECTILE: &str = "SupW_AuroraFuelAirBomb";
/// Retail AcceptableAimDelta residual (degrees).
pub const AURORA_BOMB_ACCEPTABLE_AIM_DELTA_DEG: f32 = 45.0;
/// Retail RadiusDamageAffects residual tokens.
pub const AURORA_BOMB_RADIUS_AFFECTS: &str = "ALLIES ENEMIES NEUTRALS NOT_SIMILAR";
/// Retail ProjectileCollidesWith residual.
pub const AURORA_BOMB_PROJECTILE_COLLIDES_WITH: &str = "STRUCTURES";
/// Retail FireFX residual name.
pub const AURORA_BOMB_FIRE_FX: &str = "FX_AuroraBombLaunch";
/// Retail ProjectileDetonationFX residual name.
pub const AURORA_BOMB_DETONATION_FX: &str = "FX_AuroraBombDetonate";
/// Retail WeaponSpeed residual (instant-ish bookkeeping).
pub const AURORA_BOMB_WEAPON_SPEED: f32 = 99999.0;
/// Retail AirF primary impact damage residual (tiny; OCL detonation is real blast).
pub const AIRF_AURORA_PRIMARY_DAMAGE: f32 = 2.0;
/// Retail AirF primary impact radius residual.
pub const AIRF_AURORA_PRIMARY_RADIUS: f32 = 4.0;
/// Retail SupW detonation weapon name residual.
pub const SUPW_FUEL_BOMB_DETONATION_WEAPON: &str = "SupW_FuelBombDetonationWeapon";
/// Retail AirF detonation weapon name residual.
pub const AIRF_AURORA_BOMB_DETONATION_WEAPON: &str = "AirF_AuroraBombDetonationWeapon";

// --- Wave 61 projectile residual (Object AuroraBomb) ---

/// Retail AuroraBomb projectile MaxHealth residual.
pub const AURORA_BOMB_PROJECTILE_MAX_HEALTH: f32 = 100.0;
/// Retail AuroraBomb PhysicsBehavior Mass residual.
pub const AURORA_BOMB_PROJECTILE_MASS: f32 = 75.0;
/// Retail AerodynamicFriction residual.
pub const AURORA_BOMB_PROJECTILE_AERO_FRICTION: f32 = 2.0;
/// Retail ForwardFriction residual.
pub const AURORA_BOMB_PROJECTILE_FORWARD_FRICTION: f32 = 2.0;
/// Retail CenterOfMassOffset residual.
pub const AURORA_BOMB_PROJECTILE_COM_OFFSET: f32 = 2.0;
/// Retail GeometryMajorRadius residual.
pub const AURORA_BOMB_PROJECTILE_GEOMETRY_RADIUS: f32 = 2.0;
/// Retail AuroraBombLocomotor Speed residual.
pub const AURORA_BOMB_LOCO_SPEED: f32 = 480.0;
/// Retail AuroraBombLocomotor MinSpeed residual.
pub const AURORA_BOMB_LOCO_MIN_SPEED: f32 = 240.0;
/// Retail AuroraBombLocomotor Acceleration residual.
pub const AURORA_BOMB_LOCO_ACCEL: f32 = 960.0;
/// Retail AuroraBombLocomotor TurnRate residual (deg/sec).
pub const AURORA_BOMB_LOCO_TURN_RATE: f32 = 960.0;
/// Retail AuroraBombLocomotor MaxThrustAngle residual (deg).
pub const AURORA_BOMB_LOCO_MAX_THRUST_ANGLE: f32 = 60.0;
/// Retail MissileAIUpdate TryToFollowTarget residual.
pub const AURORA_BOMB_TRY_TO_FOLLOW_TARGET: bool = false;

// --- Wave 61 AmericaJetAurora body / JetAIUpdate residual ---

/// Retail AmericaJetAurora MaxHealth residual.
pub const AURORA_JET_MAX_HEALTH: f32 = 80.0;
/// Retail VisionRange residual.
pub const AURORA_JET_VISION_RANGE: f32 = 180.0;
/// Retail ShroudClearingRange residual.
pub const AURORA_JET_SHROUD_CLEARING_RANGE: f32 = 600.0;
/// Retail BuildCost residual.
pub const AURORA_JET_BUILD_COST: u32 = 2500;
/// Retail BuildTime residual (seconds).
pub const AURORA_JET_BUILD_TIME: f32 = 30.0;
/// Retail ReturnToBaseIdleTime residual (msec).
pub const AURORA_JET_RETURN_TO_BASE_IDLE_MS: u32 = 10_000;
/// ReturnToBaseIdleTime 10000ms → 300 frames @ 30 FPS.
pub const AURORA_JET_RETURN_TO_BASE_IDLE_FRAMES: u32 = 300;
/// Retail SneakyOffsetWhenAttacking residual.
pub const AURORA_JET_SNEAKY_OFFSET: f32 = -20.0;
/// Retail AttackLocomotorPersistTime residual (msec).
pub const AURORA_JET_ATTACK_LOCO_PERSIST_MS: u32 = 100;
/// Retail AttackersMissPersistTime residual (msec).
pub const AURORA_JET_ATTACKERS_MISS_PERSIST_MS: u32 = 2000;

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

/// Convert msec residual → logic frames @ 30 FPS.
pub fn aurora_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * AURORA_LOGIC_FPS / 1000.0).round() as u32
}

/// Host residual Aurora bomb kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostAuroraBombKind {
    /// Standard AmericaJetAurora AuroraBombWeapon residual (400 dmg / r20).
    Standard,
    /// Airforce General Fuel-Air residual (`AirF_AuroraBombDetonationWeapon` 1000/r100).
    FuelAir,
    /// Superweapon General Fuel-Air residual (`SupW_FuelBombDetonationWeapon` 900/r70).
    FuelAirSupW,
}

impl HostAuroraBombKind {
    pub fn label(self) -> &'static str {
        match self {
            HostAuroraBombKind::Standard => "AuroraBomb",
            HostAuroraBombKind::FuelAir => "AuroraFuelAir",
            HostAuroraBombKind::FuelAirSupW => "AuroraFuelAirSupW",
        }
    }

    /// Absolute delay frames from activate → area damage.
    pub fn impact_delay_frames(self) -> u32 {
        match self {
            HostAuroraBombKind::Standard => AURORA_BOMB_DIVE_DELAY_FRAMES,
            HostAuroraBombKind::FuelAir | HostAuroraBombKind::FuelAirSupW => {
                AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES
            }
        }
    }

    /// Primary residual blast damage.
    pub fn damage(self) -> f32 {
        match self {
            HostAuroraBombKind::Standard => AURORA_BOMB_DAMAGE,
            HostAuroraBombKind::FuelAir => AURORA_FUEL_AIR_DAMAGE,
            HostAuroraBombKind::FuelAirSupW => AURORA_FUEL_AIR_SUPW_DAMAGE,
        }
    }

    /// Primary residual blast radius.
    pub fn radius(self) -> f32 {
        match self {
            HostAuroraBombKind::Standard => AURORA_BOMB_RADIUS,
            HostAuroraBombKind::FuelAir => AURORA_FUEL_AIR_RADIUS,
            HostAuroraBombKind::FuelAirSupW => AURORA_FUEL_AIR_SUPW_RADIUS,
        }
    }

    /// Inner radius with full damage (two-stage falloff residual).
    pub fn falloff_inner(self) -> f32 {
        match self {
            HostAuroraBombKind::Standard => AURORA_BOMB_RADIUS * 0.5,
            HostAuroraBombKind::FuelAir => AURORA_FUEL_AIR_RADIUS * 0.5,
            HostAuroraBombKind::FuelAirSupW => AURORA_FUEL_AIR_SUPW_RADIUS * 0.5,
        }
    }

    pub fn activate_audio(self) -> &'static str {
        AURORA_BOMB_LAUNCH_AUDIO
    }

    pub fn impact_audio(self) -> &'static str {
        match self {
            HostAuroraBombKind::Standard => AURORA_BOMB_DETONATE_AUDIO,
            HostAuroraBombKind::FuelAir | HostAuroraBombKind::FuelAirSupW => {
                AURORA_FUEL_AIR_DETONATE_AUDIO
            }
        }
    }

    /// Whether impact also applies retail `DaisyCutterFlameWeapon` secondary residual
    /// (AirF_AuroraBombGas / SupW_AuroraFuelAirGas SlowDeath MIDPOINT flame).
    pub fn spawns_daisy_cutter_flame(self) -> bool {
        matches!(
            self,
            HostAuroraBombKind::FuelAir | HostAuroraBombKind::FuelAirSupW
        )
    }

    /// Whether this is any Fuel-Air residual path (AirF or SupW).
    pub fn is_fuel_air(self) -> bool {
        matches!(
            self,
            HostAuroraBombKind::FuelAir | HostAuroraBombKind::FuelAirSupW
        )
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
    if n == "testaurora"
        || n == "test_aurora"
        || n == "testaurorafuelair"
        || n == "testaurorafuelairsupw"
    {
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
/// - Superweapon General → `FuelAirSupW` (`SupW_FuelBombDetonationWeapon` 900/r70)
/// - Airforce General / generic FuelAir test → `FuelAir` (`AirF` 1000/r100)
/// - Standard AmericaJetAurora → `Standard` (`AuroraBombWeapon` 400/r20)
pub fn aurora_bomb_kind_for_template(template_name: &str) -> HostAuroraBombKind {
    let n = template_name.to_ascii_lowercase();
    // SupW path first so SupW FuelAir is not collapsed into AirF numbers.
    if n.starts_with("supw") || n.contains("supw_") || n == "testaurorafuelairsupw" {
        return HostAuroraBombKind::FuelAirSupW;
    }
    if n == "testaurorafuelair"
        || n.starts_with("airf")
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
            if mission.phase != HostAuroraBombPhase::Queued || current_frame < mission.impact_frame
            {
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
                // DaisyCutterFlameWeapon secondary residual (FuelAir / SupW gas MIDPOINT).
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
                self.objects_destroyed = self.objects_destroyed.saturating_add(objects_destroyed);
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
/// ClipSize residual = 1; ClipReloadTime 5000ms honesty (full RETURN_TO_BASE airfield
/// rearm path fail-closed — host uses long reload seconds residual).
pub fn aurora_bomb_weapon(kind: HostAuroraBombKind) -> super::Weapon {
    // Host residual: weapon.damage is residual bookkeeping only — area damage is
    // applied after dive delay, not as instant single-target take_damage.
    // Use small residual damage for any path that still reads weapon.damage.
    let bookkeeping_damage = match kind {
        HostAuroraBombKind::Standard => AURORA_BOMB_DAMAGE,
        // AirF / SupW primary is intentionally tiny (2.0); detonation is residual OCL.
        HostAuroraBombKind::FuelAir | HostAuroraBombKind::FuelAirSupW => AIRF_AURORA_PRIMARY_DAMAGE,
    };
    super::Weapon {
        damage: bookkeeping_damage,
        range: AURORA_BOMB_ATTACK_RANGE,
        min_range: 0.0,
        reload_time: (AURORA_BOMB_CLIP_RELOAD_MS as f32) / 1000.0, // ClipReloadTime 5000 ms residual
        last_fire_time: -100.0,
        ammo: Some(AURORA_BOMB_CLIP_SIZE),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: AURORA_BOMB_WEAPON_SPEED,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

// --- Wave 61 residual honesty packs ---

/// Wave 61 residual honesty: AuroraBombWeapon damage / range matrix.
pub fn honesty_aurora_bomb_damage_range_residual_ok() -> bool {
    (AURORA_BOMB_DAMAGE - 400.0).abs() < 0.01
        && (AURORA_BOMB_RADIUS - 20.0).abs() < 0.01
        && (AURORA_BOMB_ATTACK_RANGE - 300.0).abs() < 0.1
        && AURORA_BOMB_DIVE_DELAY_FRAMES == 45
        && (AURORA_FUEL_AIR_DAMAGE - 1000.0).abs() < 0.01
        && (AURORA_FUEL_AIR_RADIUS - 100.0).abs() < 0.01
        && (AURORA_FUEL_AIR_SUPW_DAMAGE - 900.0).abs() < 0.01
        && (AURORA_FUEL_AIR_SUPW_RADIUS - 70.0).abs() < 0.01
        && AURORA_FUEL_AIR_GAS_DELAY_FRAMES == 30
        && AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES == 75
        && (AURORA_FUEL_AIR_FLAME_DAMAGE - 5.0).abs() < 0.01
        && (AURORA_FUEL_AIR_FLAME_RADIUS - 100.0).abs() < 0.1
        && (AIRF_AURORA_PRIMARY_DAMAGE - 2.0).abs() < 0.01
        && (AIRF_AURORA_PRIMARY_RADIUS - 4.0).abs() < 0.01
        && (AURORA_BOMB_ACCEPTABLE_AIM_DELTA_DEG - 45.0).abs() < 0.01
        && AURORA_BOMB_DAMAGE_TYPE == "AURORA_BOMB"
        && AURORA_BOMB_DEATH_TYPE == "EXPLODED"
        && AURORA_BOMB_RADIUS_AFFECTS.contains("NOT_SIMILAR")
        && AURORA_BOMB_RADIUS_AFFECTS.contains("ALLIES")
        && AURORA_BOMB_PRIMARY_WEAPON == "AuroraBombWeapon"
        && AIRF_AURORA_BOMB_PRIMARY_WEAPON == "AirF_AuroraBombWeapon"
        && SUPW_AURORA_FUEL_BOMB_WEAPON == "SupW_AuroraFuelBombWeapon"
        && AIRF_AURORA_BOMB_DETONATION_WEAPON == "AirF_AuroraBombDetonationWeapon"
        && SUPW_FUEL_BOMB_DETONATION_WEAPON == "SupW_FuelBombDetonationWeapon"
        && HostAuroraBombKind::Standard.damage() == AURORA_BOMB_DAMAGE
        && HostAuroraBombKind::FuelAir.damage() == AURORA_FUEL_AIR_DAMAGE
        && HostAuroraBombKind::FuelAirSupW.damage() == AURORA_FUEL_AIR_SUPW_DAMAGE
}

/// Wave 61 residual honesty: RETURN_TO_BASE clip reload residual.
pub fn honesty_aurora_return_to_base_residual_ok() -> bool {
    AURORA_BOMB_CLIP_SIZE == 1
        && AURORA_BOMB_CLIP_RELOAD_MS == 5000
        && AURORA_BOMB_CLIP_RELOAD_FRAMES == aurora_ms_to_frames(AURORA_BOMB_CLIP_RELOAD_MS)
        && AURORA_BOMB_CLIP_RELOAD_FRAMES == 150
        && AURORA_BOMB_AUTO_RELOADS == "RETURN_TO_BASE"
        && AURORA_JET_RETURN_TO_BASE_IDLE_MS == 10_000
        && AURORA_JET_RETURN_TO_BASE_IDLE_FRAMES
            == aurora_ms_to_frames(AURORA_JET_RETURN_TO_BASE_IDLE_MS)
        && AURORA_JET_RETURN_TO_BASE_IDLE_FRAMES == 300
        && {
            let w = aurora_bomb_weapon(HostAuroraBombKind::Standard);
            w.ammo == Some(1)
                && (w.reload_time - 5.0).abs() < 0.01
                && (w.range - AURORA_BOMB_ATTACK_RANGE).abs() < 0.01
                && !w.can_target_air
                && w.can_target_ground
        }
}

/// Wave 61 residual honesty: AuroraBomb projectile / locomotor residual.
pub fn honesty_aurora_projectile_residual_ok() -> bool {
    AURORA_BOMB_PROJECTILE == "AuroraBomb"
        && AIRF_AURORA_BOMB_PROJECTILE == "AirF_AuroraBomb"
        && SUPW_AURORA_FUEL_AIR_BOMB_PROJECTILE == "SupW_AuroraFuelAirBomb"
        && (AURORA_BOMB_PROJECTILE_MAX_HEALTH - 100.0).abs() < 0.01
        && (AURORA_BOMB_PROJECTILE_MASS - 75.0).abs() < 0.01
        && (AURORA_BOMB_PROJECTILE_AERO_FRICTION - 2.0).abs() < 0.01
        && (AURORA_BOMB_PROJECTILE_FORWARD_FRICTION - 2.0).abs() < 0.01
        && (AURORA_BOMB_PROJECTILE_COM_OFFSET - 2.0).abs() < 0.01
        && (AURORA_BOMB_PROJECTILE_GEOMETRY_RADIUS - 2.0).abs() < 0.01
        && (AURORA_BOMB_LOCO_SPEED - 480.0).abs() < 0.01
        && (AURORA_BOMB_LOCO_MIN_SPEED - 240.0).abs() < 0.01
        && (AURORA_BOMB_LOCO_ACCEL - 960.0).abs() < 0.01
        && (AURORA_BOMB_LOCO_TURN_RATE - 960.0).abs() < 0.01
        && (AURORA_BOMB_LOCO_MAX_THRUST_ANGLE - 60.0).abs() < 0.01
        && !AURORA_BOMB_TRY_TO_FOLLOW_TARGET
        && AURORA_BOMB_PROJECTILE_COLLIDES_WITH == "STRUCTURES"
        && AURORA_BOMB_FIRE_FX == "FX_AuroraBombLaunch"
        && AURORA_BOMB_DETONATION_FX == "FX_AuroraBombDetonate"
        && (AURORA_JET_MAX_HEALTH - 80.0).abs() < 0.01
        && (AURORA_JET_VISION_RANGE - 180.0).abs() < 0.01
        && (AURORA_JET_SHROUD_CLEARING_RANGE - 600.0).abs() < 0.01
        && AURORA_JET_BUILD_COST == 2500
        && (AURORA_JET_BUILD_TIME - 30.0).abs() < 0.01
        && (AURORA_JET_SNEAKY_OFFSET - (-20.0)).abs() < 0.01
        && AURORA_JET_ATTACK_LOCO_PERSIST_MS == 100
        && AURORA_JET_ATTACKERS_MISS_PERSIST_MS == 2000
}

/// Combined Wave 61 Aurora bomb residual honesty pack.
pub fn honesty_aurora_bomb_residual_pack_ok() -> bool {
    honesty_aurora_bomb_damage_range_residual_ok()
        && honesty_aurora_return_to_base_residual_ok()
        && honesty_aurora_projectile_residual_ok()
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
        // SupW_FuelBombDetonationWeapon residual matrix (not collapsed into AirF).
        assert!((AURORA_FUEL_AIR_SUPW_DAMAGE - 900.0).abs() < 0.01);
        assert!((AURORA_FUEL_AIR_SUPW_RADIUS - 70.0).abs() < 0.01);
        assert_eq!(AURORA_FUEL_AIR_GAS_DELAY_FRAMES, 30);
        assert_eq!(AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES, 75);
        assert!((AURORA_BOMB_ATTACK_RANGE - 300.0).abs() < 0.1);
        // DaisyCutterFlameWeapon secondary residual (FuelAir gas MIDPOINT).
        assert!((AURORA_FUEL_AIR_FLAME_DAMAGE - 5.0).abs() < 0.01);
        assert!((AURORA_FUEL_AIR_FLAME_RADIUS - 100.0).abs() < 0.1);
        assert!(HostAuroraBombKind::FuelAir.spawns_daisy_cutter_flame());
        assert!(HostAuroraBombKind::FuelAirSupW.spawns_daisy_cutter_flame());
        assert!(!HostAuroraBombKind::Standard.spawns_daisy_cutter_flame());
        assert!(HostAuroraBombKind::FuelAir.is_fuel_air());
        assert!(HostAuroraBombKind::FuelAirSupW.is_fuel_air());
        assert!(!HostAuroraBombKind::Standard.is_fuel_air());
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
            HostAuroraBombKind::FuelAirSupW
        );
        assert_eq!(
            aurora_bomb_kind_for_template("TestAuroraFuelAir"),
            HostAuroraBombKind::FuelAir
        );
        assert_eq!(
            aurora_bomb_kind_for_template("TestAuroraFuelAirSupW"),
            HostAuroraBombKind::FuelAirSupW
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

        // SupW_FuelBombDetonationWeapon residual: 900 / r70 (not AirF 1000/r100).
        let supw = HostAuroraBombKind::FuelAirSupW;
        assert!((aurora_bomb_damage_at_distance(supw, 0.0) - 900.0).abs() < 0.01);
        assert!((aurora_bomb_damage_at_distance(supw, 20.0) - 900.0).abs() < 0.01);
        assert_eq!(aurora_bomb_damage_at_distance(supw, 71.0), 0.0);
        let mid_supw = aurora_bomb_damage_at_distance(supw, 50.0);
        assert!(
            mid_supw > 0.0 && mid_supw < 900.0,
            "SupW mid falloff expected, got {mid_supw}"
        );
    }

    #[test]
    fn supw_fuel_bomb_900_r70_matrix_residual_honesty() {
        let mut reg = HostAuroraBombRegistry::new();
        let id = reg.queue(
            HostAuroraBombKind::FuelAirSupW,
            ObjectId(1),
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
            0,
        );
        assert_eq!(
            reg.get(id).unwrap().impact_frame,
            AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES
        );
        // Enemy at epicenter: primary 900 + flame 5.
        // Enemy at r80: outside SupW primary 70 but inside flame 100 → flame only.
        // Enemy at r120: outside both.
        let objects = vec![
            (ObjectId(1), Vec3::new(500.0, 0.0, 0.0), Team::USA, true),
            (ObjectId(2), Vec3::new(0.0, 0.0, 0.0), Team::GLA, true),
            (ObjectId(3), Vec3::new(80.0, 0.0, 0.0), Team::GLA, true),
            (ObjectId(4), Vec3::new(120.0, 0.0, 0.0), Team::GLA, true),
            (ObjectId(5), Vec3::new(0.0, 0.0, 0.0), Team::USA, true), // ally ALLIES
        ];
        let plans = reg.plan_due_impacts(AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES, &objects);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].kind, HostAuroraBombKind::FuelAirSupW);
        let hits = &plans[0].hits;
        let epic = hits
            .iter()
            .find(|h| h.target_id == ObjectId(2))
            .expect("epicenter enemy");
        let outer_primary = hits
            .iter()
            .find(|h| h.target_id == ObjectId(3))
            .expect("r80 flame-only");
        let ally = hits
            .iter()
            .find(|h| h.target_id == ObjectId(5))
            .expect("ally ALLIES residual");
        assert!(
            !hits.iter().any(|h| h.target_id == ObjectId(4)),
            "beyond primary+flame must not hit"
        );
        let expected_epic = AURORA_FUEL_AIR_SUPW_DAMAGE + AURORA_FUEL_AIR_FLAME_DAMAGE;
        assert!(
            (epic.damage - expected_epic).abs() < 0.1,
            "SupW epicenter must be 900+5, got {}",
            epic.damage
        );
        assert!(
            (ally.damage - expected_epic).abs() < 0.1,
            "SupW ally epicenter residual"
        );
        // Outside SupW r70 primary but inside flame r100 → flame only.
        assert!(
            (outer_primary.damage - AURORA_FUEL_AIR_FLAME_DAMAGE).abs() < 0.1,
            "r80 must be flame-only (outside SupW r70 primary), got {}",
            outer_primary.damage
        );
        // Contrast: AirF at same r80 still has primary falloff (r100).
        assert!(
            aurora_bomb_damage_at_distance(HostAuroraBombKind::FuelAir, 80.0)
                > AURORA_FUEL_AIR_FLAME_DAMAGE,
            "AirF r80 still in primary radius — matrix must differ from SupW"
        );
        reg.record_impact_complete(id, expected_epic * 2.0, 2, 0);
        assert!(reg.honesty_complete_ok_of_kind(HostAuroraBombKind::FuelAirSupW));
        assert!(reg.honesty_host_path_ok());
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
        assert_eq!(
            plans[0].hits.len(),
            2,
            "enemy + ally at epicenter (ALLIES residual)"
        );
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
        let mid = hits
            .iter()
            .find(|h| h.target_id == ObjectId(11))
            .expect("mid ally");
        let epic = hits
            .iter()
            .find(|h| h.target_id == ObjectId(13))
            .expect("epic enemy");
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
        assert!((w.reload_time - 5.0).abs() < 0.01);
    }

    #[test]
    fn aurora_bomb_residual_pack_honesty_wave61() {
        assert!(honesty_aurora_bomb_damage_range_residual_ok());
        assert!(honesty_aurora_return_to_base_residual_ok());
        assert!(honesty_aurora_projectile_residual_ok());
        assert!(honesty_aurora_bomb_residual_pack_ok());
        assert_eq!(aurora_ms_to_frames(5000), 150);
        assert_eq!(aurora_ms_to_frames(10_000), 300);
        assert_eq!(AURORA_BOMB_AUTO_RELOADS, "RETURN_TO_BASE");
        assert_eq!(AURORA_BOMB_PROJECTILE, "AuroraBomb");
        assert!((AURORA_BOMB_LOCO_SPEED - 480.0).abs() < 0.01);
        assert!((AURORA_JET_MAX_HEALTH - 80.0).abs() < 0.01);
        assert_eq!(AURORA_JET_BUILD_COST, 2500);
    }
}
