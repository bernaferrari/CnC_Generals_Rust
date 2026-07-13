//! Host base-defense structure residual (Patriot / Gattling / Stinger auto-fire).
//!
//! Residual slice (playability):
//! - Base defenses (USA Patriot, China Gattling Cannon, GLA Stinger Site,
//!   GLA Tunnel Network gun, and `FSBaseDefense` structures) auto-acquire and
//!   damage nearby enemies while Idle without a manual `AttackObject` / player
//!   attack order.
//! - Retail weapon names:
//!   - `PatriotMissileWeapon` (dmg 30, range 225) + SECONDARY `PatriotMissileWeaponAir`
//!     (dmg 25, range 350, AA)
//!   - Laser General residual: `Lazr_PatriotMissileWeapon` (dmg **40** / r**3**)
//!     + air residual `Lazr_PatriotMissileWeaponAir` (dmg **35** / r**3** / range **350**)
//!     (retail SECONDARY assist slot collapsed; TERTIARY air → residual secondary)
//!   - Superweapon General residual: `SupW_PatriotMissileWeapon` (dmg **15** /
//!     range **275**) + air `SupW_PatriotMissileWeaponAir` (dmg **30** / range **400**)
//!     + EMPPatriotEffectSpheroid residual (DISABLED_EMP **10000** ms / r**10**)
//!   - `GattlingBuildingGun` (dmg 10, range 225) + SECONDARY `GattlingBuildingGunAir`
//!     (dmg 5, range 400, AA only)
//!   - Stinger residual (SPAWNS_ARE_THE_WEAPONS abstraction): structure fires
//!     `StingerMissileWeapon` (dmg 20, range 225) + SECONDARY `StingerMissileWeaponAir`
//!     (dmg 30, range 400, AA) as the site's 3 slave soldiers would.
//!   - GLA Tunnel Network residual: PRIMARY `TunnelNetworkGun` (dmg **15** /
//!     range **175** / Delay **250**ms → 8 frames). Ground residual only.
//! - China Gattling Cannon continuous-fire ramp residual (`FiringTracker`):
//!   - ContinuousFireOne=**1** / Two=**5** / Coast=**2000**ms (60 frames)
//!   - Base Delay **250**ms (8 frames) → MEAN **4** (200% RoF) → FAST **2** (300% RoF)
//!   - Chain Guns PLAYER_UPGRADE residual (`Upgrade_ChinaChainGuns`): damage × **1.25**
//! - GLA AP Rockets PLAYER_UPGRADE residual (`Upgrade_GLAAPRockets`): Stinger damage × **1.25**
//! - C++ `AIUpdateInterface` AutoAcquireEnemiesWhenIdle residual for stationary
//!   base defenses (not full turret pitch / LOS).
//!
//! - **AssistedTargetingUpdate residual** (AmericaPatriotBattery ModuleTag_07):
//!   When a Patriot fires PRIMARY/AA with `RequestAssistRange` **200**, same-team
//!   equivalent Patriots within range that are free to assist fire
//!   `AssistingClipSize` **4** shots of the assist weapon (retail SECONDARY
//!   `PatriotMissileAssistWeapon` / Lazr / SupW variants, AttackRange **450**).
//!   Host residual processes the assist clip over DelayBetweenShots **250**ms
//!   (**8** frames) cadence.
//!
//! - **BinaryDataStream laser residual** (`LaserFromAssisted` / `LaserToTarget`):
//!   On assist accept, host residual spawns two short-lived feedback beams using
//!   retail template `PatriotBinaryDataStream` (DeletionUpdate **600**ms → **18**
//!   frames): requestor→assistant and assistant→victim.
//!
//! - **LaserUpdate endpoint tracking residual**: each logic frame, residual
//!   refreshes beam start/end from live `from_id` / `to_id` object positions
//!   (C++ `LaserUpdate::updateStartPos` / `updateEndPos` without bone matrix).
//!   Dead/missing target freezes the last end point residual. W3DLaserDraw
//!   residual honesty: NumBeams **1**, ScrollRate **-0.25**, ArcHeight **30**,
//!   InnerBeamWidth **4**, Segments **20**; host advances scroll residual by
//!   ScrollRate each frame while active.
//!
//! - **HiveStructureBody + SpawnBehavior residual** (GLAStingerSite):
//!   SpawnNumber **3** residual soldiers (MaxHealth **100** each). Propagate
//!   SMALL_ARMS/SNIPER/POISON/RADIATION/SURRENDER/MICROWAVE residual damage to the
//!   active slave; swallow SNIPER/POISON/SURRENDER when no slaves remain; all
//!   other damage hits the structure. SPAWNS_ARE_THE_WEAPONS residual: site cannot
//!   fire with **0** soldiers. SpawnReplaceDelay **30000**ms → **900** frames.
//!
//! - **Physical SpawnBehavior slave roster + getClosestSlave residual**:
//!   Host tracks **3** residual slave slots at SpawnPoint bone offsets (radius
//!   **12**, 120° layout). `getClosestSlave` residual picks the alive slave
//!   nearest the shooter in 2D; HiveStructureBody propagate damages that slot.
//!   Respawn revives the first dead slot. Fail-closed: not full GLAInfantryStingerSoldier
//!   Object spawn / AI / W3D model attach.
//!
//! - **W3DLaserDraw arc segment residual** (PatriotBinaryDataStream):
//!   Host samples cosine arc points from start→end using retail ArcHeight **30**
//!   / Segments **20** (C++ `doDrawModule` mid-peak cos curve). Fail-closed:
//!   not full texture / Line3D GPU draw.
//!
//! Fail-closed honesty:
//! - Not full WeaponSet PRIMARY/SECONDARY/TERTIARY chooser beyond air/ground residual
//!   (assist SECONDARY is residual-separate; host dual-slot still maps AA to residual
//!   secondary for auto-acquire)
//! - Not full W3DLaserDraw texture / Line3D GPU draw for assist beams
//!   (endpoint track + draw-param + arc segment sample host residual closed)
//! - Not full SpawnBehavior physical soldier Object / AI / bone matrix
//!   (getClosestSlave + per-slave HP/position host residual closed)
//! - Not full PointDefenseLaserUpdate missile intercept matrix
//! - Not full CONTINUOUS_FIRE_* model-condition animation / VoiceRapidFire matrix
//! - Not network base-defense replication (network deferred)

use super::{ObjectId, Weapon};
use crate::game_logic::host_gattling_tank::{GattlingFireLevel, GATTLING_CHAIN_GUN_DAMAGE_MULT};
use std::collections::HashSet;

/// Retail Patriot primary weapon template name.
pub const PATRIOT_PRIMARY_WEAPON: &str = "PatriotMissileWeapon";
/// Retail Patriot secondary AA weapon template name.
pub const PATRIOT_SECONDARY_WEAPON: &str = "PatriotMissileWeaponAir";
/// Retail Laser General Patriot primary residual.
pub const LAZR_PATRIOT_PRIMARY_WEAPON: &str = "Lazr_PatriotMissileWeapon";
/// Retail Laser General Patriot AA residual (TERTIARY → residual secondary slot).
pub const LAZR_PATRIOT_SECONDARY_WEAPON: &str = "Lazr_PatriotMissileWeaponAir";
/// Retail Superweapon General Patriot primary residual (EMP missiles).
pub const SUPW_PATRIOT_PRIMARY_WEAPON: &str = "SupW_PatriotMissileWeapon";
/// Retail Superweapon General Patriot AA residual.
pub const SUPW_PATRIOT_SECONDARY_WEAPON: &str = "SupW_PatriotMissileWeaponAir";

/// Retail PatriotMissileWeapon PrimaryDamage.
pub const PATRIOT_GROUND_DAMAGE: f32 = 30.0;
/// Retail Lazr_PatriotMissileWeapon PrimaryDamage residual.
pub const LAZR_PATRIOT_GROUND_DAMAGE: f32 = 40.0;
/// Retail SupW_PatriotMissileWeapon PrimaryDamage residual.
pub const SUPW_PATRIOT_GROUND_DAMAGE: f32 = 15.0;
/// Retail PatriotMissileWeapon AttackRange.
pub const PATRIOT_GROUND_RANGE: f32 = 225.0;
/// Retail SupW_PatriotMissileWeapon AttackRange residual.
pub const SUPW_PATRIOT_GROUND_RANGE: f32 = 275.0;
/// Retail PatriotMissileWeaponAir PrimaryDamage.
pub const PATRIOT_AIR_DAMAGE: f32 = 25.0;
/// Retail Lazr_PatriotMissileWeaponAir PrimaryDamage residual.
pub const LAZR_PATRIOT_AIR_DAMAGE: f32 = 35.0;
/// Retail SupW_PatriotMissileWeaponAir PrimaryDamage residual.
pub const SUPW_PATRIOT_AIR_DAMAGE: f32 = 30.0;
/// Retail PatriotMissileWeaponAir AttackRange.
pub const PATRIOT_AIR_RANGE: f32 = 350.0;
/// Retail SupW_PatriotMissileWeaponAir AttackRange residual.
pub const SUPW_PATRIOT_AIR_RANGE: f32 = 400.0;
/// Retail Patriot DelayBetweenShots 250ms → 8 frames @ 30 FPS (in-clip).
pub const PATRIOT_DELAY_FRAMES: u32 = 8;
/// Retail Patriot ClipReloadTime 2000ms → 60 frames residual between clips.
/// Fail-closed host residual: use clip-reload as effective shot cadence.
pub const PATRIOT_CLIP_RELOAD_FRAMES: u32 = 60;
/// Residual fire audio for Patriot.
pub const PATRIOT_FIRE_AUDIO: &str = "PatriotBatteryWeapon";
/// Residual Laser General Patriot fire audio honesty.
pub const LAZR_PATRIOT_FIRE_AUDIO: &str = "Lazr_WeaponFX_LaserCrusader";

// --- AssistedTargetingUpdate residual (Patriot ModuleTag_07) ---
/// Retail PatriotMissileWeapon / Air `RequestAssistRange`.
pub const PATRIOT_REQUEST_ASSIST_RANGE: f32 = 200.0;
/// Retail AssistedTargetingUpdate `AssistingClipSize`.
pub const PATRIOT_ASSISTING_CLIP_SIZE: u32 = 4;
/// Retail PatriotMissileAssistWeapon AttackRange.
pub const PATRIOT_ASSIST_RANGE: f32 = 450.0;
/// Retail PatriotMissileAssistWeapon / SupW_PatriotMissileAssistWeapon PrimaryDamage.
pub const PATRIOT_ASSIST_DAMAGE: f32 = 25.0;
/// Retail Lazr_PatriotMissileAssistWeapon PrimaryDamage residual.
pub const LAZR_PATRIOT_ASSIST_DAMAGE: f32 = 35.0;
/// Retail assist DelayBetweenShots 250ms → 8 frames @ 30 FPS.
pub const PATRIOT_ASSIST_DELAY_FRAMES: u32 = 8;
/// Retail assist ClipReloadTime 1000ms → 30 frames residual honesty.
pub const PATRIOT_ASSIST_CLIP_RELOAD_FRAMES: u32 = 30;
/// Retail assist weapon template names (honesty / docs).
pub const PATRIOT_ASSIST_WEAPON: &str = "PatriotMissileAssistWeapon";
pub const LAZR_PATRIOT_ASSIST_WEAPON: &str = "Lazr_PatriotMissileAssistWeapon";
pub const SUPW_PATRIOT_ASSIST_WEAPON: &str = "SupW_PatriotMissileAssistWeapon";
/// Residual BinaryDataStream laser honesty cue (not full LaserUpdate drawable).
pub const PATRIOT_ASSIST_LASER_AUDIO: &str = "PatriotBinaryDataStream";
/// Retail `LaserFromAssisted` / `LaserToTarget` ThingTemplate name.
pub const PATRIOT_BINARY_DATA_STREAM: &str = "PatriotBinaryDataStream";
/// Retail PatriotBinaryDataStream DeletionUpdate Min/MaxLifetime **600**ms → **18** frames @ 30 FPS.
pub const PATRIOT_ASSIST_LASER_LIFETIME_FRAMES: u32 = 18;

// --- PatriotBinaryDataStream W3DLaserDraw residual honesty (draw params) ---
/// Retail W3DLaserDraw NumBeams.
pub const PATRIOT_LASER_NUM_BEAMS: u32 = 1;
/// Retail W3DLaserDraw InnerBeamWidth.
pub const PATRIOT_LASER_INNER_BEAM_WIDTH: f32 = 4.0;
/// Retail W3DLaserDraw ScrollRate (towards = negative).
pub const PATRIOT_LASER_SCROLL_RATE: f32 = -0.25;
/// Retail W3DLaserDraw Segments.
pub const PATRIOT_LASER_SEGMENTS: u32 = 20;
/// Retail W3DLaserDraw ArcHeight.
pub const PATRIOT_LASER_ARC_HEIGHT: f32 = 30.0;
/// Retail W3DLaserDraw TilingScalar.
pub const PATRIOT_LASER_TILING_SCALAR: f32 = 0.25;
/// Retail W3DLaserDraw SegmentOverlapRatio default (host residual honesty).
pub const PATRIOT_LASER_SEGMENT_OVERLAP_RATIO: f32 = 0.0;

/// Kind of residual assist feedback laser (AssistedTargetingUpdate::makeFeedbackLaser).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatriotAssistLaserKind {
    /// `LaserFromAssisted`: stream from the requestor to the assisting Patriot.
    FromAssisted,
    /// `LaserToTarget`: stream from the assisting Patriot to the victim.
    ToTarget,
}

/// Host residual BinaryDataStream laser beam (LaserUpdate endpoint track residual).
#[derive(Debug, Clone, PartialEq)]
pub struct ResidualPatriotAssistLaser {
    pub kind: PatriotAssistLaserKind,
    pub from_id: ObjectId,
    pub to_id: ObjectId,
    pub from_x: f32,
    pub from_y: f32,
    pub from_z: f32,
    pub to_x: f32,
    pub to_y: f32,
    pub to_z: f32,
    /// Absolute logic frame when DeletionUpdate residual expires the beam.
    pub expires_frame: u32,
    /// W3DLaserDraw ScrollRate residual accum (starts 0; advances by ScrollRate/frame).
    pub scroll_offset: f32,
    /// Residual honesty: endpoint was refreshed from live parent/target at least once.
    pub endpoint_tracked: bool,
}

impl ResidualPatriotAssistLaser {
    /// Retail template name residual for both assist laser kinds.
    pub fn template_name(&self) -> &'static str {
        PATRIOT_BINARY_DATA_STREAM
    }

    /// Whether the residual beam is still live at `frame`.
    pub fn is_active_at(&self, frame: u32) -> bool {
        frame < self.expires_frame
    }

    /// Retail W3DLaserDraw NumBeams residual honesty.
    pub fn num_beams(&self) -> u32 {
        PATRIOT_LASER_NUM_BEAMS
    }

    /// Retail W3DLaserDraw ArcHeight residual honesty.
    pub fn arc_height(&self) -> f32 {
        PATRIOT_LASER_ARC_HEIGHT
    }

    /// Retail W3DLaserDraw InnerBeamWidth residual honesty.
    pub fn inner_beam_width(&self) -> f32 {
        PATRIOT_LASER_INNER_BEAM_WIDTH
    }

    /// Retail W3DLaserDraw Segments residual honesty.
    pub fn segments(&self) -> u32 {
        PATRIOT_LASER_SEGMENTS
    }
}

/// Absolute frame when a residual assist laser expires (start + 18 frames).
pub fn patriot_assist_laser_expires_frame(start_frame: u32) -> u32 {
    start_frame.saturating_add(PATRIOT_ASSIST_LASER_LIFETIME_FRAMES.max(1))
}

/// Build the two residual BinaryDataStream lasers spawned on assist accept.
///
/// C++ `AssistedTargetingUpdate::assistAttack`:
/// - `makeFeedbackLaser(LaserFromAssisted, requestingObject, me)`
/// - `makeFeedbackLaser(LaserToTarget, me, victimObject)`
pub fn make_patriot_assist_lasers(
    requester_id: ObjectId,
    assistant_id: ObjectId,
    victim_id: ObjectId,
    requester_pos: (f32, f32, f32),
    assistant_pos: (f32, f32, f32),
    victim_pos: (f32, f32, f32),
    start_frame: u32,
) -> [ResidualPatriotAssistLaser; 2] {
    let expires = patriot_assist_laser_expires_frame(start_frame);
    [
        ResidualPatriotAssistLaser {
            kind: PatriotAssistLaserKind::FromAssisted,
            from_id: requester_id,
            to_id: assistant_id,
            from_x: requester_pos.0,
            from_y: requester_pos.1,
            from_z: requester_pos.2,
            to_x: assistant_pos.0,
            to_y: assistant_pos.1,
            to_z: assistant_pos.2,
            expires_frame: expires,
            scroll_offset: 0.0,
            endpoint_tracked: false,
        },
        ResidualPatriotAssistLaser {
            kind: PatriotAssistLaserKind::ToTarget,
            from_id: assistant_id,
            to_id: victim_id,
            from_x: assistant_pos.0,
            from_y: assistant_pos.1,
            from_z: assistant_pos.2,
            to_x: victim_pos.0,
            to_y: victim_pos.1,
            to_z: victim_pos.2,
            expires_frame: expires,
            scroll_offset: 0.0,
            endpoint_tracked: false,
        },
    ]
}

/// C++ LaserUpdate::clientUpdate residual: refresh endpoints from live objects
/// and advance W3DLaserDraw ScrollRate residual. Missing/dead `to` freezes end.
///
/// `lookup` returns `(x, y, z, alive)` for an ObjectId when present.
/// Returns how many beams had an endpoint position change this frame.
pub fn track_patriot_assist_laser_endpoints<F>(
    lasers: &mut [ResidualPatriotAssistLaser],
    mut lookup: F,
) -> u32
where
    F: FnMut(ObjectId) -> Option<(f32, f32, f32, bool)>,
{
    let mut moved = 0u32;
    for laser in lasers.iter_mut() {
        laser.scroll_offset += PATRIOT_LASER_SCROLL_RATE;
        let mut changed = false;
        if let Some((x, y, z, alive)) = lookup(laser.from_id) {
            if alive {
                if (laser.from_x - x).abs() > 1e-4
                    || (laser.from_y - y).abs() > 1e-4
                    || (laser.from_z - z).abs() > 1e-4
                {
                    changed = true;
                }
                laser.from_x = x;
                laser.from_y = y;
                laser.from_z = z;
            }
        }
        if let Some((x, y, z, alive)) = lookup(laser.to_id) {
            if alive {
                if (laser.to_x - x).abs() > 1e-4
                    || (laser.to_y - y).abs() > 1e-4
                    || (laser.to_z - z).abs() > 1e-4
                {
                    changed = true;
                }
                laser.to_x = x;
                laser.to_y = y;
                laser.to_z = z;
            }
            // Dead/missing target: freeze last end (fail-closed vs punchThroughScalar).
        }
        if changed {
            laser.endpoint_tracked = true;
            moved = moved.saturating_add(1);
        }
    }
    moved
}

/// Retain only residual assist lasers still active at `frame`.
pub fn expire_patriot_assist_lasers(
    lasers: &mut Vec<ResidualPatriotAssistLaser>,
    frame: u32,
) -> u32 {
    let before = lasers.len();
    lasers.retain(|l| l.is_active_at(frame));
    (before.saturating_sub(lasers.len())) as u32
}

// --- W3DLaserDraw arc segment residual (C++ doDrawModule cos curve) ---

/// C++ W3DLaserDraw arc height boost at normalized `t` ∈ [0, 1] along the beam.
///
/// Retail: `height = cos(dist_from_mid / half_length * π/2) * ArcHeight`.
/// Midpoint (t=0.5) → full ArcHeight; endpoints (t=0/1) → 0.
pub fn patriot_laser_arc_z_boost(t: f32, arc_height: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let dist_ratio = (t - 0.5).abs() * 2.0; // 0 at mid → 1 at ends
    let scaled = dist_ratio * std::f32::consts::FRAC_PI_2; // 0 → π/2
    arc_height * scaled.cos()
}

/// Sample a residual W3DLaserDraw arc point at segment start ratio.
///
/// C++ builds Segments residual ground samples then raises Z by cos-arc.
/// Host residual: lerp start→end XY + linear Z base, then add arc boost.
/// Fail-closed: not ground-height skimming / SegmentOverlapRatio stretch.
pub fn sample_patriot_laser_arc_point(
    from: (f32, f32, f32),
    to: (f32, f32, f32),
    t: f32,
    arc_height: f32,
) -> (f32, f32, f32) {
    let t = t.clamp(0.0, 1.0);
    let x = from.0 + (to.0 - from.0) * t;
    let y = from.1 + (to.1 - from.1) * t;
    let z_base = from.2 + (to.2 - from.2) * t;
    let z = z_base + patriot_laser_arc_z_boost(t, arc_height);
    (x, y, z)
}

/// Sample residual arc segment endpoints for segment `i` of `segments`.
///
/// Returns `(start, end)` world points for the residual Line3D segment.
pub fn sample_patriot_laser_arc_segment(
    from: (f32, f32, f32),
    to: (f32, f32, f32),
    segment: u32,
    segments: u32,
    arc_height: f32,
) -> ((f32, f32, f32), (f32, f32, f32)) {
    let segs = segments.max(1) as f32;
    let i = segment.min(segments.saturating_sub(1)) as f32;
    let t0 = i / segs;
    let t1 = (i + 1.0) / segs;
    (
        sample_patriot_laser_arc_point(from, to, t0, arc_height),
        sample_patriot_laser_arc_point(from, to, t1, arc_height),
    )
}

/// Midpoint residual arc peak Z boost (should equal ArcHeight on level beams).
pub fn patriot_laser_arc_peak_boost(arc_height: f32) -> f32 {
    patriot_laser_arc_z_boost(0.5, arc_height)
}

// --- SupW EMPPatriotEffectSpheroid residual (ProjectileDetonationOCL) ---
/// Retail EMPPatriotEffectSpheroid EffectRadius residual.
pub const SUPW_PATRIOT_EMP_RADIUS: f32 = 10.0;
/// Retail EMPPatriotEffectSpheroid DisabledDuration 10000 ms → 300 frames @ 30 FPS.
pub const SUPW_PATRIOT_EMP_DURATION_FRAMES: u32 = 300;
/// Residual EMP impact audio honesty.
pub const SUPW_PATRIOT_EMP_AUDIO: &str = "EMPPulseWhoosh";

/// Retail Stinger soldier primary (structure residual abstraction).
pub const STINGER_PRIMARY_WEAPON: &str = "StingerMissileWeapon";
/// Retail Stinger soldier secondary AA (structure residual abstraction).
pub const STINGER_SECONDARY_WEAPON: &str = "StingerMissileWeaponAir";

/// Retail StingerMissileWeapon PrimaryDamage.
pub const STINGER_GROUND_DAMAGE: f32 = 20.0;
/// Retail StingerMissileWeapon AttackRange.
pub const STINGER_GROUND_RANGE: f32 = 225.0;
/// Retail StingerMissileWeaponAir PrimaryDamage.
pub const STINGER_AIR_DAMAGE: f32 = 30.0;
/// Retail StingerMissileWeaponAir AttackRange.
pub const STINGER_AIR_RANGE: f32 = 400.0;
/// Retail ClipReloadTime 2000ms → 60 frames @ 30 FPS (ClipSize=1).
pub const STINGER_RELOAD_FRAMES: u32 = 60;
/// Retail SpawnBehavior SpawnNumber for residual honesty (not full spawn).
pub const STINGER_SPAWN_NUMBER: u32 = 3;
/// Retail GLAInfantryStingerSoldier MaxHealth residual.
pub const STINGER_SOLDIER_MAX_HEALTH: f32 = 100.0;
/// Retail SpawnReplaceDelay 30000ms → 900 frames @ 30 FPS.
pub const STINGER_SPAWN_REPLACE_DELAY_FRAMES: u32 = 900;
/// Host residual SpawnPoint bone radius (W3D SpawnPoint layout residual).
///
/// Fail-closed: not full model bone matrix — three soldiers ring the site.
pub const STINGER_SPAWN_POINT_RADIUS: f32 = 12.0;
/// Residual SpawnTemplate honesty name (stock Stinger Site).
pub const STINGER_SPAWN_TEMPLATE: &str = "GLAInfantryStingerSoldier";
/// Residual fire audio for Stinger residual shots.
pub const STINGER_FIRE_AUDIO: &str = "StingerMissileWeapon";
/// Residual soldier death audio honesty (not full OCL / FXListDie).
pub const STINGER_SOLDIER_DIE_AUDIO: &str = "StingerSoldierVoiceDie";
/// Retail Upgrade_GLAAPRockets WeaponBonus DAMAGE 125%.
pub const UPGRADE_GLA_AP_ROCKETS: &str = "Upgrade_GLAAPRockets";
/// AP Rockets damage multiplier residual.
pub const STINGER_AP_ROCKETS_DAMAGE_MULT: f32 = 1.25;

// --- HiveStructureBody residual (Stinger Site ModuleTag_04) ---

/// Host residual damage class for HiveStructureBody::attemptDamage.
///
/// Retail Stinger Site:
/// - PropagateDamageTypesToSlavesWhenExisting = SMALL_ARMS + SNIPER + POISON +
///   RADIATION + SURRENDER + MICROWAVE
/// - SwallowDamageTypesIfSlavesNotExisting = SNIPER + POISON + SURRENDER
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostHiveDamageClass {
    /// Damage types that route to residual slaves when present.
    PropagateToSlaves,
    /// Subset that is swallowed (no structure damage) when no slaves remain.
    SwallowIfNoSlaves,
    /// All other damage hits the structure body residual.
    HitStructure,
}

/// Result of applying residual HiveStructureBody damage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostHiveDamageResult {
    /// Whether the structure itself was destroyed by this hit.
    pub structure_destroyed: bool,
    /// HP removed from the structure body (0 when propagated/swallowed).
    pub structure_damage_applied: f32,
    /// HP removed from the active residual slave.
    pub slave_damage_applied: f32,
    /// How many residual slaves died this hit (0 or 1 host residual).
    pub slaves_killed: u32,
    /// True when swallow residual ate the damage (no slaves + swallow class).
    pub swallowed: bool,
    /// Residual slave index damaged by getClosestSlave path (`None` if none).
    pub closest_slave_index: Option<u8>,
}

/// Host residual physical SpawnBehavior slave slot (Stinger Site).
///
/// Fail-closed: not a full Object — position offset + HP only (getClosestSlave).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResidualHiveSlave {
    /// Residual soldier HP (MaxHealth 100).
    pub hp: f32,
    /// World-XZ offset from site center (SpawnPoint bone residual).
    pub offset_x: f32,
    pub offset_z: f32,
    /// Alive residual (dead slots wait for SpawnReplaceDelay).
    pub alive: bool,
}

impl Default for ResidualHiveSlave {
    fn default() -> Self {
        Self {
            hp: 0.0,
            offset_x: 0.0,
            offset_z: 0.0,
            alive: false,
        }
    }
}

impl ResidualHiveSlave {
    /// World residual position given site center (x, z).
    pub fn world_xz(&self, site_x: f32, site_z: f32) -> (f32, f32) {
        (site_x + self.offset_x, site_z + self.offset_z)
    }
}

/// Deterministic residual SpawnPoint bone offsets (3 soldiers @ 120° ring).
pub fn stinger_spawn_point_offsets() -> [(f32, f32); 3] {
    let r = STINGER_SPAWN_POINT_RADIUS;
    let half = r * 0.5;
    let y = r * (3.0_f32).sqrt() * 0.5; // sin(120°) * r
    [(r, 0.0), (-half, y), (-half, -y)]
}

/// Build full residual slave roster for a constructed Stinger Site.
pub fn init_stinger_hive_slave_roster() -> [ResidualHiveSlave; 3] {
    let offs = stinger_spawn_point_offsets();
    [
        ResidualHiveSlave {
            hp: STINGER_SOLDIER_MAX_HEALTH,
            offset_x: offs[0].0,
            offset_z: offs[0].1,
            alive: true,
        },
        ResidualHiveSlave {
            hp: STINGER_SOLDIER_MAX_HEALTH,
            offset_x: offs[1].0,
            offset_z: offs[1].1,
            alive: true,
        },
        ResidualHiveSlave {
            hp: STINGER_SOLDIER_MAX_HEALTH,
            offset_x: offs[2].0,
            offset_z: offs[2].1,
            alive: true,
        },
    ]
}

/// Initial residual slave count + HP for a constructed Stinger Site.
pub fn init_stinger_hive_slaves() -> (u8, f32) {
    (STINGER_SPAWN_NUMBER as u8, STINGER_SOLDIER_MAX_HEALTH)
}

/// SPAWNS_ARE_THE_WEAPONS residual: site can fire only while slaves remain.
pub fn stinger_can_fire_with_slaves(slave_count: u8) -> bool {
    slave_count > 0
}

/// Count alive residual slaves.
pub fn count_alive_hive_slaves(slaves: &[ResidualHiveSlave]) -> u8 {
    slaves.iter().filter(|s| s.alive).count() as u8
}

/// Active residual slave HP (first alive) — mirror for legacy count/hp fields.
pub fn active_hive_slave_hp(slaves: &[ResidualHiveSlave]) -> f32 {
    slaves
        .iter()
        .find(|s| s.alive)
        .map(|s| s.hp)
        .unwrap_or(0.0)
}

/// Sync legacy `(count, hp)` mirrors from residual roster.
pub fn sync_hive_slave_mirrors(slaves: &[ResidualHiveSlave]) -> (u8, f32) {
    (count_alive_hive_slaves(slaves), active_hive_slave_hp(slaves))
}

/// C++ `SpawnBehavior::getClosestSlave` residual — nearest alive slave to world point.
///
/// Returns index into residual roster, or `None` when no slaves remain.
pub fn get_closest_hive_slave_index(
    slaves: &[ResidualHiveSlave],
    site_x: f32,
    site_z: f32,
    query_x: f32,
    query_z: f32,
) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, slave) in slaves.iter().enumerate() {
        if !slave.alive {
            continue;
        }
        let (sx, sz) = slave.world_xz(site_x, site_z);
        let dx = sx - query_x;
        let dz = sz - query_z;
        let d2 = dx * dx + dz * dz;
        if best.map(|(_, bd)| d2 < bd).unwrap_or(true) {
            best = Some((i, d2));
        }
    }
    best.map(|(i, _)| i)
}

/// First alive residual slave index (legacy "active" slave when no shooter).
pub fn first_alive_hive_slave_index(slaves: &[ResidualHiveSlave]) -> Option<usize> {
    slaves.iter().position(|s| s.alive)
}

/// Pure HiveStructureBody residual resolution (mutates slave count/HP inputs).
///
/// Returns updated `(slave_count, slave_hp, structure_hp_after, result)`.
/// Uses first-alive residual path (no shooter / getClosestSlave).
/// `structure_hp` is used only for HitStructure / fallback paths.
pub fn resolve_hive_structure_damage(
    slave_count: u8,
    slave_hp: f32,
    structure_hp: f32,
    damage: f32,
    class: HostHiveDamageClass,
) -> (u8, f32, f32, HostHiveDamageResult) {
    // Legacy path: synthesize a single active residual slot from count/hp.
    let mut slaves = [ResidualHiveSlave::default(); 3];
    let n = (slave_count as usize).min(3);
    for i in 0..n {
        slaves[i].alive = true;
        slaves[i].hp = if i == 0 {
            slave_hp.max(0.0)
        } else {
            STINGER_SOLDIER_MAX_HEALTH
        };
        let offs = stinger_spawn_point_offsets();
        slaves[i].offset_x = offs[i].0;
        slaves[i].offset_z = offs[i].1;
    }
    let (new_slaves, new_struct, result) =
        resolve_hive_structure_damage_roster(&mut slaves, structure_hp, damage, class, None);
    let (c, h) = sync_hive_slave_mirrors(&new_slaves);
    // When first slave dies, legacy mirror expects next slave at full HP (h).
    let _ = new_slaves;
    (c, h, new_struct, result)
}

/// HiveStructureBody residual with physical slave roster + getClosestSlave.
///
/// `shooter_xz`: when `Some`, damages closest slave to shooter (C++ path).
/// When `None`, damages first alive residual (host legacy residual).
pub fn resolve_hive_structure_damage_roster(
    slaves: &mut [ResidualHiveSlave],
    structure_hp: f32,
    damage: f32,
    class: HostHiveDamageClass,
    shooter_xz: Option<(f32, f32, f32, f32)>, // (site_x, site_z, shoot_x, shoot_z)
) -> ([ResidualHiveSlave; 3], f32, HostHiveDamageResult) {
    let mut roster = [ResidualHiveSlave::default(); 3];
    for (i, s) in slaves.iter().take(3).enumerate() {
        roster[i] = *s;
    }
    let dmg = damage.max(0.0);
    if dmg <= 0.0 {
        return (
            roster,
            structure_hp,
            HostHiveDamageResult {
                structure_destroyed: structure_hp <= 0.0,
                structure_damage_applied: 0.0,
                slave_damage_applied: 0.0,
                slaves_killed: 0,
                swallowed: false,
                closest_slave_index: None,
            },
        );
    }

    let propagate = matches!(
        class,
        HostHiveDamageClass::PropagateToSlaves | HostHiveDamageClass::SwallowIfNoSlaves
    );
    let swallow = matches!(class, HostHiveDamageClass::SwallowIfNoSlaves);
    let alive = count_alive_hive_slaves(&roster);

    if propagate && alive > 0 {
        let idx = match shooter_xz {
            Some((sx, sz, qx, qz)) => get_closest_hive_slave_index(&roster, sx, sz, qx, qz),
            None => first_alive_hive_slave_index(&roster),
        };
        if let Some(i) = idx {
            let before = roster[i].hp.max(0.0);
            let applied = dmg.min(before);
            roster[i].hp = (before - dmg).max(0.0);
            let mut killed = 0u32;
            if roster[i].hp <= 0.0 {
                roster[i].alive = false;
                roster[i].hp = 0.0;
                killed = 1;
            }
            // Write back.
            for (j, s) in slaves.iter_mut().take(3).enumerate() {
                *s = roster[j];
            }
            return (
                roster,
                structure_hp,
                HostHiveDamageResult {
                    structure_destroyed: false,
                    structure_damage_applied: 0.0,
                    slave_damage_applied: applied,
                    slaves_killed: killed,
                    swallowed: false,
                    closest_slave_index: Some(i as u8),
                },
            );
        }
    }

    if swallow && alive == 0 {
        return (
            roster,
            structure_hp,
            HostHiveDamageResult {
                structure_destroyed: false,
                structure_damage_applied: 0.0,
                slave_damage_applied: 0.0,
                slaves_killed: 0,
                swallowed: true,
                closest_slave_index: None,
            },
        );
    }

    // Structure body residual.
    let new_hp = (structure_hp - dmg).max(0.0);
    let applied = structure_hp - new_hp;
    (
        roster,
        new_hp,
        HostHiveDamageResult {
            structure_destroyed: new_hp <= 0.0,
            structure_damage_applied: applied,
            slave_damage_applied: 0.0,
            slaves_killed: 0,
            swallowed: false,
            closest_slave_index: None,
        },
    )
}

/// Respawn one residual dead slave slot (SpawnReplaceDelay). Returns true if respawned.
pub fn respawn_one_hive_slave(slaves: &mut [ResidualHiveSlave]) -> bool {
    if let Some(slot) = slaves.iter_mut().find(|s| !s.alive) {
        slot.alive = true;
        slot.hp = STINGER_SOLDIER_MAX_HEALTH;
        // Keep existing offset (SpawnPoint residual).
        true
    } else {
        false
    }
}

/// Schedule next residual slave respawn after a death (SpawnReplaceDelay).
pub fn next_stinger_slave_respawn_frame(current_frame: u32, already_scheduled: u32) -> u32 {
    if already_scheduled > current_frame {
        already_scheduled
    } else {
        current_frame.saturating_add(STINGER_SPAWN_REPLACE_DELAY_FRAMES)
    }
}

/// Whether a residual slave should respawn this frame.
pub fn should_respawn_stinger_slave(
    slave_count: u8,
    current_frame: u32,
    respawn_frame: u32,
) -> bool {
    slave_count < STINGER_SPAWN_NUMBER as u8 && respawn_frame > 0 && current_frame >= respawn_frame
}

/// Retail China Gattling Cannon primary weapon template name.
pub const GATTLING_BUILDING_PRIMARY_WEAPON: &str = "GattlingBuildingGun";
/// Retail China Gattling Cannon secondary AA weapon template name.
pub const GATTLING_BUILDING_SECONDARY_WEAPON: &str = "GattlingBuildingGunAir";

/// Retail GattlingBuildingGun PrimaryDamage.
pub const GATTLING_BUILDING_GROUND_DAMAGE: f32 = 10.0;
/// Retail GattlingBuildingGun AttackRange.
pub const GATTLING_BUILDING_GROUND_RANGE: f32 = 225.0;
/// Retail GattlingBuildingGunAir PrimaryDamage.
pub const GATTLING_BUILDING_AIR_DAMAGE: f32 = 5.0;
/// Retail GattlingBuildingGunAir AttackRange.
pub const GATTLING_BUILDING_AIR_RANGE: f32 = 400.0;

/// Retail DelayBetweenShots 250ms → 8 frames @ 30 FPS.
pub const GATTLING_BUILDING_BASE_DELAY_FRAMES: u32 = 8;
/// ContinuousFireOne for building gun (retail = 1).
pub const GATTLING_BUILDING_CONTINUOUS_FIRE_ONE: u32 = 1;
/// ContinuousFireTwo for building gun (retail = 5).
pub const GATTLING_BUILDING_CONTINUOUS_FIRE_TWO: u32 = 5;
/// ContinuousFireCoast 2000ms → 60 frames @ 30 FPS.
pub const GATTLING_BUILDING_COAST_FRAMES: u32 = 60;

/// Residual fire audio for structure gattling.
pub const GATTLING_BUILDING_FIRE_AUDIO: &str = "GattlingCannonWeapon";
/// Retail VoiceRapidFire residual cue when entering FAST.
pub const GATTLING_BUILDING_RAPID_FIRE_AUDIO: &str = "GattlingCannonVoiceRapid";

/// Whether template is a residual base-defense structure that should auto-fire.
///
/// Fail-closed: name + FSBaseDefense kind residual (not full INI module matrix).
/// Excludes Overlord/Helix/tank-mounted gattling payloads (not structures).
pub fn is_base_defense_structure(
    template_name: &str,
    is_structure: bool,
    is_fs_base_defense: bool,
) -> bool {
    if is_fs_base_defense {
        return true;
    }
    if !is_structure {
        return false;
    }
    let n = template_name.to_ascii_lowercase();
    // Vehicle/portable gattling payloads are not structure base defenses.
    if n.contains("overlord") || n.contains("helix") || n.contains("tank") || n.contains("gunship")
    {
        return false;
    }
    n.contains("patriot")
        || n.contains("gattlingcannon")
        || n.contains("gattling_cannon")
        || n.contains("stingersite")
        || n.contains("stinger_site")
        || n.contains("basedefense")
        || n.contains("base_defense")
        || n.contains("firebase")
        // GLA Tunnel Network gun residual (enter/exit residual already host-closed).
        || crate::game_logic::host_tunnel_network::is_tunnel_network_template(template_name)
}

/// Whether template is a residual USA Patriot battery (ground + AA residual).
///
/// Fail-closed: name residual. Excludes projectile / weapon / debris names.
pub fn is_patriot_battery_structure(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Reject pure weapon / projectile / debris names.
    if n.contains("weapon")
        || n.contains("projectile")
        || n.starts_with("upgrade")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
    {
        return false;
    }
    // Known host / retail / general-variant structure names.
    matches!(
        n.as_str(),
        "usa_patriot"
            | "usa_patriotmissile"
            | "americapatriotbattery"
            | "patriotmissile"
            | "testpatriot"
            | "testlazrpatriot"
            | "testsupwpatriot"
            | "testemppatriot"
    ) || (n.contains("patriot")
        && (n.contains("battery")
            || n.contains("system")
            || n.starts_with("usa")
            || n.starts_with("america")
            || n.starts_with("lazr_")
            || n.starts_with("airf_")
            || n.starts_with("supw_")
            || n.starts_with("testlazr")
            || n.starts_with("testsupw")
            || n.starts_with("testemp")))
}

/// Whether template is a residual GLA Stinger Site (SPAWNS_ARE_THE_WEAPONS residual).
///
/// Fail-closed: name residual. Excludes soldier / weapon / hole debris.
pub fn is_stinger_site_structure(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("soldier")
        || n.contains("weapon")
        || n.contains("missile")
        || n.contains("projectile")
        || n.starts_with("upgrade")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("hole")
        || n.contains("dead")
    {
        return false;
    }
    n.contains("stingersite")
        || n.contains("stinger_site")
        || n == "gla_stingersite"
        || n == "teststingersite"
        || (n.contains("stinger") && n.contains("site"))
}

/// Whether template is a residual China Gattling Cannon structure (ramp + AA).
///
/// Fail-closed: name residual. Excludes tank / Overlord / Helix payloads.
pub fn is_gattling_cannon_structure(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Weapons / upgrades / science / debris.
    if n.contains("weapon")
        || n.contains("gun")
        || n.contains("shell")
        || n.contains("projectile")
        || n.contains("missile")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("training")
        || n.contains("dead")
        || n.contains("hulk")
        || n.contains("debris")
    {
        return false;
    }
    // Portable Overlord/Helix payloads are not the structure residual.
    if n.contains("overlord") || n.contains("helix") {
        return false;
    }
    // Vehicle gattling tanks (ChinaTankGattling / *Vehicle*Gattling*) are host_gattling_tank.
    // General-variant buildings keep a `Tank_` / `Nuke_` prefix and still match *GattlingCannon*.
    if (n.contains("gattlingtank") || n.contains("gatlingtank") || n.contains("tankgattling"))
        && !n.contains("cannon")
    {
        return false;
    }
    if n.contains("vehiclegattling") || n.contains("vehiclegatling") {
        return false;
    }
    n.contains("gattlingcannon")
        || n.contains("gatlingcannon")
        || n.contains("gattling_cannon")
        || n.contains("gatling_cannon")
        || n == "china_gattlingcannon"
        || n == "testgattlingcannon"
        || n == "testgatlingcannon"
}

/// Whether template is a Laser General Patriot residual (Lazr_ prefix).
///
/// Fail-closed: name residual (not full general production gate).
pub fn is_laser_patriot_template(template_name: &str) -> bool {
    if !is_patriot_battery_structure(template_name) {
        return false;
    }
    let n = template_name.to_ascii_lowercase();
    n.starts_with("lazr_")
        || n.contains("lazr_patriot")
        || n.contains("lazr_america")
        || n == "testlazrpatriot"
}

/// Whether template is a Superweapon General EMP Patriot residual (SupW_ prefix).
///
/// Fail-closed: name residual (not full general production gate / EMP drawable).
pub fn is_supw_patriot_template(template_name: &str) -> bool {
    if !is_patriot_battery_structure(template_name) {
        return false;
    }
    let n = template_name.to_ascii_lowercase();
    n.starts_with("supw_")
        || n.contains("supw_patriot")
        || n.contains("supw_america")
        || n == "testsupwpatriot"
        || n == "testemppatriot"
}

/// Absolute frame when SupW Patriot EMP residual expires.
pub fn supw_patriot_emp_until_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(SUPW_PATRIOT_EMP_DURATION_FRAMES)
}

/// Whether residual target is legal for SupW Patriot EMP disable residual.
///
/// Retail EMPPatriotEffectSpheroid EMPUpdate: vehicles / faction structures /
/// SPAWNS_ARE_THE_WEAPONS; DoesNotAffectMyOwnBuildings residual skips own structures.
pub fn is_legal_supw_patriot_emp_target(
    is_vehicle: bool,
    is_aircraft: bool,
    is_faction_structure: bool,
    is_own_structure: bool,
    is_alive: bool,
    under_construction: bool,
    is_emp_hardened: bool,
) -> bool {
    if !is_alive || under_construction || is_emp_hardened {
        return false;
    }
    // Own buildings not disabled residual (DoesNotAffectMyOwnBuildings = Yes).
    if is_own_structure {
        return false;
    }
    is_vehicle || is_aircraft || is_faction_structure
}

/// Retail-ish residual weapon name for known host base-defense templates.
pub fn primary_weapon_name_for_defense(template_name: &str) -> Option<&'static str> {
    if is_patriot_battery_structure(template_name) {
        Some(if is_laser_patriot_template(template_name) {
            LAZR_PATRIOT_PRIMARY_WEAPON
        } else if is_supw_patriot_template(template_name) {
            SUPW_PATRIOT_PRIMARY_WEAPON
        } else {
            PATRIOT_PRIMARY_WEAPON
        })
    } else if is_gattling_cannon_structure(template_name) {
        Some(GATTLING_BUILDING_PRIMARY_WEAPON)
    } else if is_stinger_site_structure(template_name) {
        Some(STINGER_PRIMARY_WEAPON)
    } else if crate::game_logic::host_tunnel_network::is_tunnel_network_template(template_name) {
        Some(crate::game_logic::host_tunnel_network::TUNNEL_NETWORK_GUN)
    } else {
        None
    }
}

/// Secondary AA residual weapon name for dual-slot base defenses.
pub fn secondary_weapon_name_for_defense(template_name: &str) -> Option<&'static str> {
    if is_gattling_cannon_structure(template_name) {
        Some(GATTLING_BUILDING_SECONDARY_WEAPON)
    } else if is_patriot_battery_structure(template_name) {
        Some(if is_laser_patriot_template(template_name) {
            LAZR_PATRIOT_SECONDARY_WEAPON
        } else if is_supw_patriot_template(template_name) {
            SUPW_PATRIOT_SECONDARY_WEAPON
        } else {
            PATRIOT_SECONDARY_WEAPON
        })
    } else if is_stinger_site_structure(template_name) {
        Some(STINGER_SECONDARY_WEAPON)
    } else {
        None
    }
}

/// Whether this base defense uses dual ground/AA residual slots.
pub fn is_dual_slot_base_defense(template_name: &str) -> bool {
    is_gattling_cannon_structure(template_name)
        || is_stinger_site_structure(template_name)
        || is_patriot_battery_structure(template_name)
}

/// Slot residual for dual air/ground base defenses: 1 = AA secondary, 0 = ground primary.
pub fn preferred_dual_defense_slot(target_is_air: bool) -> u8 {
    preferred_gattling_building_slot(target_is_air)
}

/// Whether AP Rockets upgrade is active on a Stinger residual host.
pub fn stinger_has_ap_rockets(applied_upgrades: &HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l == UPGRADE_GLA_AP_ROCKETS.to_ascii_lowercase()
            || l.contains("aprockets")
            || l.contains("ap_rockets")
    })
}

/// Apply AP Rockets residual damage mult when upgrade present.
pub fn stinger_damage_with_ap_rockets(base_damage: f32, has_ap: bool) -> f32 {
    if has_ap {
        base_damage * STINGER_AP_ROCKETS_DAMAGE_MULT
    } else {
        base_damage
    }
}

/// Build residual Stinger ground Weapon (soldier PRIMARY residual).
pub fn stinger_ground_weapon(has_ap_rockets: bool) -> Weapon {
    Weapon {
        damage: stinger_damage_with_ap_rockets(STINGER_GROUND_DAMAGE, has_ap_rockets),
        range: STINGER_GROUND_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(STINGER_RELOAD_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(1),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 750.0,
        pre_attack_delay: 0.0,
    }
}

/// Build residual Stinger AA Weapon (soldier SECONDARY residual).
pub fn stinger_air_weapon(has_ap_rockets: bool) -> Weapon {
    Weapon {
        damage: stinger_damage_with_ap_rockets(STINGER_AIR_DAMAGE, has_ap_rockets),
        range: STINGER_AIR_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(STINGER_RELOAD_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(1),
        can_target_air: true,
        can_target_ground: false,
        projectile_speed: 600.0,
        pre_attack_delay: 0.0,
    }
}

/// Build residual Patriot ground Weapon (standard shell residual).
pub fn patriot_ground_weapon() -> Weapon {
    patriot_ground_weapon_for_template("AmericaPatriotBattery")
}

/// Build residual Patriot AA Weapon (standard shell residual).
pub fn patriot_air_weapon() -> Weapon {
    patriot_air_weapon_for_template("AmericaPatriotBattery")
}

/// Build residual Patriot ground Weapon for a specific battery template.
pub fn patriot_ground_weapon_for_template(template_name: &str) -> Weapon {
    let laser = is_laser_patriot_template(template_name);
    let supw = is_supw_patriot_template(template_name);
    Weapon {
        damage: if laser {
            LAZR_PATRIOT_GROUND_DAMAGE
        } else if supw {
            SUPW_PATRIOT_GROUND_DAMAGE
        } else {
            PATRIOT_GROUND_DAMAGE
        },
        range: if supw {
            SUPW_PATRIOT_GROUND_RANGE
        } else {
            PATRIOT_GROUND_RANGE
        },
        min_range: 0.0,
        // Fail-closed: effective cadence ≈ clip reload (ClipSize residual not full matrix).
        // Lazr ClipSize=3 residual collapses to same clip-reload honesty as stock.
        reload_time: delay_frames_to_reload_secs(PATRIOT_CLIP_RELOAD_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(if laser { 3 } else { 4 }),
        can_target_air: false,
        can_target_ground: true,
        // Instant laser residual vs missile residual.
        projectile_speed: if laser { 999_999.0 } else { 600.0 },
        pre_attack_delay: 0.0,
    }
}

/// Build residual Patriot AA Weapon for a specific battery template.
pub fn patriot_air_weapon_for_template(template_name: &str) -> Weapon {
    let laser = is_laser_patriot_template(template_name);
    let supw = is_supw_patriot_template(template_name);
    Weapon {
        damage: if laser {
            LAZR_PATRIOT_AIR_DAMAGE
        } else if supw {
            SUPW_PATRIOT_AIR_DAMAGE
        } else {
            PATRIOT_AIR_DAMAGE
        },
        range: if supw {
            SUPW_PATRIOT_AIR_RANGE
        } else {
            PATRIOT_AIR_RANGE
        },
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(PATRIOT_CLIP_RELOAD_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(4),
        can_target_air: true,
        can_target_ground: false,
        projectile_speed: if laser { 999_999.0 } else { 600.0 },
        pre_attack_delay: 0.0,
    }
}

/// Residual assist damage for a Patriot template family (stock / Lazr / SupW).
pub fn patriot_assist_damage_for_template(template_name: &str) -> f32 {
    if is_laser_patriot_template(template_name) {
        LAZR_PATRIOT_ASSIST_DAMAGE
    } else {
        // Stock + SupW assist shells both use PrimaryDamage 25 residual.
        PATRIOT_ASSIST_DAMAGE
    }
}

/// Residual assist weapon name honesty for a Patriot template family.
pub fn patriot_assist_weapon_name_for_template(template_name: &str) -> &'static str {
    if is_laser_patriot_template(template_name) {
        LAZR_PATRIOT_ASSIST_WEAPON
    } else if is_supw_patriot_template(template_name) {
        SUPW_PATRIOT_ASSIST_WEAPON
    } else {
        PATRIOT_ASSIST_WEAPON
    }
}

/// Whether two Patriot residual templates are "equivalent" for assist requests.
///
/// C++ `ThingTemplate::isEquivalentTo` residual: same general family (stock / Lazr /
/// SupW). Fail-closed: not full reskin / faction-building inheritance matrix.
pub fn patriots_are_assist_equivalent(requester: &str, assistant: &str) -> bool {
    if !is_patriot_battery_structure(requester) || !is_patriot_battery_structure(assistant) {
        return false;
    }
    is_laser_patriot_template(requester) == is_laser_patriot_template(assistant)
        && is_supw_patriot_template(requester) == is_supw_patriot_template(assistant)
}

/// Whether a Patriot is free to answer an AssistedTargeting residual request.
///
/// C++ `AssistedTargetingUpdate::isFreeToAssist`: able to attack + current weapon
/// READY_TO_FIRE. Host residual: constructed, can attack, not under construction,
/// and not already mid-assist clip.
pub fn is_patriot_free_to_assist(
    is_alive: bool,
    is_constructed: bool,
    can_attack: bool,
    under_construction: bool,
    already_assisting: bool,
    weapon_ready: bool,
) -> bool {
    is_alive
        && is_constructed
        && can_attack
        && !under_construction
        && !already_assisting
        && weapon_ready
}

/// Whether assistant is within RequestAssistRange of the requesting Patriot.
pub fn is_within_patriot_request_assist_range(dist_2d: f32) -> bool {
    dist_2d <= PATRIOT_REQUEST_ASSIST_RANGE
}

/// Whether victim is within assist weapon AttackRange from the assistant.
pub fn is_within_patriot_assist_weapon_range(dist_2d: f32) -> bool {
    dist_2d <= PATRIOT_ASSIST_RANGE
}

/// Pending assist clip residual (AssistingClipSize shots at DelayBetweenShots).
#[derive(Debug, Clone, PartialEq)]
pub struct PendingPatriotAssist {
    pub assistant_id: ObjectId,
    pub victim_id: ObjectId,
    pub requester_id: ObjectId,
    pub shots_remaining: u32,
    pub next_shot_frame: u32,
    /// Template name snapshot for damage / EMP family residual.
    pub assistant_template: String,
}

impl PendingPatriotAssist {
    pub fn new(
        assistant_id: ObjectId,
        victim_id: ObjectId,
        requester_id: ObjectId,
        start_frame: u32,
        assistant_template: impl Into<String>,
    ) -> Self {
        Self {
            assistant_id,
            victim_id,
            requester_id,
            shots_remaining: PATRIOT_ASSISTING_CLIP_SIZE,
            // First shot is immediate (C++ assistAttack locks and fires ASAP).
            next_shot_frame: start_frame,
            assistant_template: assistant_template.into(),
        }
    }

    pub fn damage(&self) -> f32 {
        patriot_assist_damage_for_template(&self.assistant_template)
    }

    pub fn is_supw(&self) -> bool {
        is_supw_patriot_template(&self.assistant_template)
    }
}

/// Legal residual target for base-defense auto-fire.
pub fn is_legal_base_defense_target(
    is_alive: bool,
    same_team: bool,
    is_neutral: bool,
    under_construction: bool,
    is_attackable_or_combat_kind: bool,
) -> bool {
    is_alive && !same_team && !is_neutral && !under_construction && is_attackable_or_combat_kind
}

/// Slot residual for structure gattling: 1 = AA secondary, 0 = ground primary.
pub fn preferred_gattling_building_slot(target_is_air: bool) -> u8 {
    if target_is_air {
        1
    } else {
        0
    }
}

/// Delay frames residual for continuous-fire level (base / ROF).
///
/// C++ uses floor(delay / ROF). Residual:
/// - Base: 8
/// - Mean: floor(8/2)=4
/// - Fast: floor(8/3)=2
pub fn gattling_building_delay_frames_for_level(level: GattlingFireLevel) -> u32 {
    let base = GATTLING_BUILDING_BASE_DELAY_FRAMES as f32;
    let rof = level.rof_multiplier();
    (base / rof).floor().max(1.0) as u32
}

/// Apply Chain Guns residual damage mult when upgrade present.
pub fn gattling_building_damage_with_chain_guns(base_damage: f32, has_chain_guns: bool) -> f32 {
    if has_chain_guns {
        base_damage * GATTLING_CHAIN_GUN_DAMAGE_MULT
    } else {
        base_damage
    }
}

fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Ground gun residual stats (damage, range, delay_frames) for level + chain guns.
pub fn gattling_building_ground_stats(
    level: GattlingFireLevel,
    has_chain_guns: bool,
) -> (f32, f32, u32) {
    let dmg =
        gattling_building_damage_with_chain_guns(GATTLING_BUILDING_GROUND_DAMAGE, has_chain_guns);
    (
        dmg,
        GATTLING_BUILDING_GROUND_RANGE,
        gattling_building_delay_frames_for_level(level),
    )
}

/// Air gun residual stats (damage, range, delay_frames) for level + chain guns.
pub fn gattling_building_air_stats(
    level: GattlingFireLevel,
    has_chain_guns: bool,
) -> (f32, f32, u32) {
    let dmg =
        gattling_building_damage_with_chain_guns(GATTLING_BUILDING_AIR_DAMAGE, has_chain_guns);
    (
        dmg,
        GATTLING_BUILDING_AIR_RANGE,
        gattling_building_delay_frames_for_level(level),
    )
}

/// Build residual ground Weapon for level + chain guns.
pub fn gattling_building_ground_weapon(level: GattlingFireLevel, has_chain_guns: bool) -> Weapon {
    let (dmg, range, delay) = gattling_building_ground_stats(level, has_chain_guns);
    Weapon {
        damage: dmg,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
    }
}

/// Build residual air Weapon for level + chain guns.
pub fn gattling_building_air_weapon(level: GattlingFireLevel, has_chain_guns: bool) -> Weapon {
    let (dmg, range, delay) = gattling_building_air_stats(level, has_chain_guns);
    Weapon {
        damage: dmg,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: true,
        can_target_ground: false,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
    }
}

/// Advance continuous-fire residual state after a structure gattling shot.
///
/// Mirrors C++ `FiringTracker::shotFired` with building thresholds
/// ContinuousFireOne=1 / ContinuousFireTwo=5.
/// Returns `(new_level, consecutive, entered_fast)`.
pub fn gattling_building_on_shot_fired(
    previous_level: GattlingFireLevel,
    previous_consecutive: u32,
    previous_victim: Option<u32>,
    new_victim: Option<u32>,
    current_frame: u32,
    coast_until_frame: u32,
) -> (GattlingFireLevel, u32, bool) {
    let same_or_within_coast = match (previous_victim, new_victim) {
        (Some(a), Some(b)) if a == b => true,
        _ if current_frame < coast_until_frame => true,
        _ => false,
    };

    let consecutive = if same_or_within_coast {
        previous_consecutive.saturating_add(1).max(1)
    } else {
        1
    };

    let mut level = previous_level;
    let mut entered_fast = false;

    match previous_level {
        GattlingFireLevel::Mean => {
            if consecutive < GATTLING_BUILDING_CONTINUOUS_FIRE_ONE {
                level = GattlingFireLevel::Base;
            } else if consecutive > GATTLING_BUILDING_CONTINUOUS_FIRE_TWO {
                level = GattlingFireLevel::Fast;
                entered_fast = true;
            }
        }
        GattlingFireLevel::Fast => {
            if consecutive < GATTLING_BUILDING_CONTINUOUS_FIRE_TWO {
                // C++ coolDown: straight to zero from FAST.
                level = GattlingFireLevel::Base;
            }
        }
        GattlingFireLevel::Base => {
            if consecutive > GATTLING_BUILDING_CONTINUOUS_FIRE_ONE {
                level = GattlingFireLevel::Mean;
            }
        }
    }

    (level, consecutive, entered_fast)
}

/// Next coast-until frame after a shot (next possible shot frame + coast residual).
pub fn gattling_building_coast_until_after_shot(
    current_frame: u32,
    level: GattlingFireLevel,
) -> u32 {
    let delay = gattling_building_delay_frames_for_level(level);
    current_frame
        .saturating_add(delay)
        .saturating_add(GATTLING_BUILDING_COAST_FRAMES)
}

/// Whether Chain Guns upgrade is active on a structure gattling residual host.
pub fn gattling_building_has_chain_guns(applied_upgrades: &HashSet<String>) -> bool {
    crate::game_logic::host_gattling_tank::has_chain_guns_upgrade(applied_upgrades)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn base_defense_name_matrix() {
        assert!(is_base_defense_structure("USA_Patriot", true, false));
        assert!(is_base_defense_structure(
            "AmericaPatriotBattery",
            true,
            false
        ));
        assert!(is_base_defense_structure(
            "Lazr_PatriotMissileSystem",
            true,
            false
        ));
        assert!(is_base_defense_structure(
            "China_GattlingCannon",
            true,
            false
        ));
        assert!(is_base_defense_structure(
            "ChinaGattlingCannon",
            true,
            false
        ));
        assert!(is_base_defense_structure("GLA_StingerSite", true, false));
        assert!(is_base_defense_structure("AnyTower", true, true));
        assert!(!is_base_defense_structure("USA_Barracks", true, false));
        assert!(!is_base_defense_structure("USA_Ranger", false, false));
        assert!(!is_base_defense_structure(
            "ChinaTankOverlordGattlingCannon",
            false,
            false
        ));
        assert!(!is_base_defense_structure(
            "ChinaHelixGattlingCannon",
            false,
            false
        ));
        assert!(!is_base_defense_structure("USA_SupplyCenter", true, false));
        assert!(is_base_defense_structure("GLATunnelNetwork", true, false));
        assert!(is_base_defense_structure("TestTunnelNetwork", true, false));
        assert!(!is_base_defense_structure(
            "GLASneakAttackTunnelNetworkStart",
            true,
            false
        ));
    }

    #[test]
    fn gattling_cannon_structure_name_matrix() {
        assert!(is_gattling_cannon_structure("China_GattlingCannon"));
        assert!(is_gattling_cannon_structure("ChinaGattlingCannon"));
        assert!(is_gattling_cannon_structure("Nuke_ChinaGattlingCannon"));
        assert!(is_gattling_cannon_structure("Tank_ChinaGattlingCannon"));
        assert!(is_gattling_cannon_structure("Infa_ChinaGattlingCannon"));
        assert!(is_gattling_cannon_structure("TestGattlingCannon"));
        // Tank residual — not structure.
        assert!(!is_gattling_cannon_structure("ChinaTankGattling"));
        assert!(!is_gattling_cannon_structure("ChinaVehicleGattlingTank"));
        // Overlord / Helix payload — not structure residual.
        assert!(!is_gattling_cannon_structure(
            "ChinaTankOverlordGattlingCannon"
        ));
        assert!(!is_gattling_cannon_structure("ChinaHelixGattlingCannon"));
        // Weapons / upgrades.
        assert!(!is_gattling_cannon_structure("GattlingBuildingGun"));
        assert!(!is_gattling_cannon_structure("GattlingBuildingGunAir"));
        assert!(!is_gattling_cannon_structure("Upgrade_ChinaChainGuns"));
        assert!(!is_gattling_cannon_structure("USA_Patriot"));
    }

    #[test]
    fn defense_weapon_name_lookup() {
        assert_eq!(
            primary_weapon_name_for_defense("USA_Patriot"),
            Some(PATRIOT_PRIMARY_WEAPON)
        );
        assert_eq!(
            secondary_weapon_name_for_defense("USA_Patriot"),
            Some(PATRIOT_SECONDARY_WEAPON)
        );
        assert_eq!(
            primary_weapon_name_for_defense("China_GattlingCannon"),
            Some(GATTLING_BUILDING_PRIMARY_WEAPON)
        );
        assert_eq!(
            secondary_weapon_name_for_defense("China_GattlingCannon"),
            Some(GATTLING_BUILDING_SECONDARY_WEAPON)
        );
        assert_eq!(
            primary_weapon_name_for_defense("GLA_StingerSite"),
            Some(STINGER_PRIMARY_WEAPON)
        );
        assert_eq!(
            secondary_weapon_name_for_defense("GLA_StingerSite"),
            Some(STINGER_SECONDARY_WEAPON)
        );
        assert_eq!(
            primary_weapon_name_for_defense("Lazr_AmericaPatriotBattery"),
            Some(LAZR_PATRIOT_PRIMARY_WEAPON)
        );
        assert_eq!(
            secondary_weapon_name_for_defense("Lazr_AmericaPatriotBattery"),
            Some(LAZR_PATRIOT_SECONDARY_WEAPON)
        );
        assert_eq!(
            primary_weapon_name_for_defense("SupW_AmericaPatriotBattery"),
            Some(SUPW_PATRIOT_PRIMARY_WEAPON)
        );
        assert_eq!(
            secondary_weapon_name_for_defense("SupW_AmericaPatriotBattery"),
            Some(SUPW_PATRIOT_SECONDARY_WEAPON)
        );
        assert_eq!(
            primary_weapon_name_for_defense("GLATunnelNetwork"),
            Some(crate::game_logic::host_tunnel_network::TUNNEL_NETWORK_GUN)
        );
        assert_eq!(primary_weapon_name_for_defense("USA_Ranger"), None);
        assert!(is_dual_slot_base_defense("USA_Patriot"));
        assert!(is_dual_slot_base_defense("Lazr_AmericaPatriotBattery"));
        assert!(is_dual_slot_base_defense("SupW_AmericaPatriotBattery"));
        assert!(is_dual_slot_base_defense("GLA_StingerSite"));
        assert!(is_dual_slot_base_defense("China_GattlingCannon"));
        assert!(!is_dual_slot_base_defense("USA_Barracks"));
        assert!(!is_dual_slot_base_defense("GLATunnelNetwork"));
    }

    #[test]
    fn laser_patriot_weapon_stats() {
        assert!(is_laser_patriot_template("Lazr_AmericaPatriotBattery"));
        assert!(is_laser_patriot_template("TestLazrPatriot"));
        assert!(!is_laser_patriot_template("AmericaPatriotBattery"));
        let g = patriot_ground_weapon_for_template("Lazr_AmericaPatriotBattery");
        assert!((g.damage - LAZR_PATRIOT_GROUND_DAMAGE).abs() < 0.01);
        let a = patriot_air_weapon_for_template("Lazr_AmericaPatriotBattery");
        assert!((a.damage - LAZR_PATRIOT_AIR_DAMAGE).abs() < 0.01);
        assert!(a.can_target_air);
        let stock = patriot_ground_weapon();
        assert!((stock.damage - PATRIOT_GROUND_DAMAGE).abs() < 0.01);
    }

    #[test]
    fn supw_patriot_emp_weapon_stats() {
        assert!(is_supw_patriot_template("SupW_AmericaPatriotBattery"));
        assert!(is_supw_patriot_template("TestSupWPatriot"));
        assert!(is_supw_patriot_template("TestEmpPatriot"));
        assert!(!is_supw_patriot_template("AmericaPatriotBattery"));
        assert!(!is_supw_patriot_template("Lazr_AmericaPatriotBattery"));
        let g = patriot_ground_weapon_for_template("SupW_AmericaPatriotBattery");
        assert!((g.damage - SUPW_PATRIOT_GROUND_DAMAGE).abs() < 0.01);
        assert!((g.range - SUPW_PATRIOT_GROUND_RANGE).abs() < 0.01);
        let a = patriot_air_weapon_for_template("SupW_AmericaPatriotBattery");
        assert!((a.damage - SUPW_PATRIOT_AIR_DAMAGE).abs() < 0.01);
        assert!((a.range - SUPW_PATRIOT_AIR_RANGE).abs() < 0.01);
        assert!(a.can_target_air);
        assert_eq!(supw_patriot_emp_until_frame(10), 310);
        assert_eq!(SUPW_PATRIOT_EMP_RADIUS, 10.0);
        assert!(is_legal_supw_patriot_emp_target(
            true, false, false, false, true, false, false
        ));
        assert!(!is_legal_supw_patriot_emp_target(
            false, false, true, true, true, false, false
        )); // own building
    }

    #[test]
    fn stinger_and_patriot_structure_name_matrix() {
        assert!(is_stinger_site_structure("GLA_StingerSite"));
        assert!(is_stinger_site_structure("GLAStingerSite"));
        assert!(is_stinger_site_structure("Chem_GLAStingerSite"));
        assert!(is_stinger_site_structure("Demo_GLAStingerSite"));
        assert!(is_stinger_site_structure("Slth_GLAStingerSite"));
        assert!(is_stinger_site_structure("TestStingerSite"));
        assert!(!is_stinger_site_structure("GLAInfantryStingerSoldier"));
        assert!(!is_stinger_site_structure("StingerMissileWeapon"));
        assert!(!is_stinger_site_structure("GLAHoleStingerSite"));

        assert!(is_patriot_battery_structure("USA_Patriot"));
        assert!(is_patriot_battery_structure("AmericaPatriotBattery"));
        assert!(is_patriot_battery_structure("Lazr_PatriotMissileSystem"));
        assert!(is_patriot_battery_structure("TestPatriot"));
        assert!(!is_patriot_battery_structure("PatriotMissileWeapon"));
        assert!(!is_patriot_battery_structure("PatriotMissileProjectile"));
    }

    #[test]
    fn stinger_and_patriot_weapon_stats() {
        let ground = stinger_ground_weapon(false);
        assert!((ground.damage - STINGER_GROUND_DAMAGE).abs() < 0.01);
        assert!((ground.range - STINGER_GROUND_RANGE).abs() < 0.01);
        assert!(!ground.can_target_air);
        assert!(ground.can_target_ground);

        let air = stinger_air_weapon(false);
        assert!((air.damage - STINGER_AIR_DAMAGE).abs() < 0.01);
        assert!((air.range - STINGER_AIR_RANGE).abs() < 0.01);
        assert!(air.can_target_air);
        assert!(!air.can_target_ground);

        let ap = stinger_ground_weapon(true);
        assert!((ap.damage - STINGER_GROUND_DAMAGE * STINGER_AP_ROCKETS_DAMAGE_MULT).abs() < 0.01);

        let mut tags = HashSet::new();
        assert!(!stinger_has_ap_rockets(&tags));
        tags.insert(UPGRADE_GLA_AP_ROCKETS.to_string());
        assert!(stinger_has_ap_rockets(&tags));

        let pg = patriot_ground_weapon();
        assert!((pg.damage - PATRIOT_GROUND_DAMAGE).abs() < 0.01);
        assert!((pg.range - PATRIOT_GROUND_RANGE).abs() < 0.01);
        assert!(!pg.can_target_air);

        let pa = patriot_air_weapon();
        assert!((pa.damage - PATRIOT_AIR_DAMAGE).abs() < 0.01);
        assert!((pa.range - PATRIOT_AIR_RANGE).abs() < 0.01);
        assert!(pa.can_target_air);
        assert!(!pa.can_target_ground);

        assert_eq!(preferred_dual_defense_slot(false), 0);
        assert_eq!(preferred_dual_defense_slot(true), 1);
        assert_eq!(STINGER_SPAWN_NUMBER, 3);
    }

    #[test]
    fn legal_target_matrix() {
        assert!(is_legal_base_defense_target(
            true, false, false, false, true
        ));
        assert!(!is_legal_base_defense_target(
            false, false, false, false, true
        ));
        assert!(!is_legal_base_defense_target(
            true, true, false, false, true
        ));
        assert!(!is_legal_base_defense_target(
            true, false, true, false, true
        ));
        assert!(!is_legal_base_defense_target(
            true, false, false, true, true
        ));
        assert!(!is_legal_base_defense_target(
            true, false, false, false, false
        ));
    }

    #[test]
    fn continuous_fire_ramp_thresholds_building() {
        // Shot 1 → consecutive 1, stay Base (need > 1).
        let (l1, c1, f1) =
            gattling_building_on_shot_fired(GattlingFireLevel::Base, 0, None, Some(10), 0, 0);
        assert_eq!(l1, GattlingFireLevel::Base);
        assert_eq!(c1, 1);
        assert!(!f1);

        // Shot 2 → consecutive 2 > 1 → Mean.
        let (l2, c2, f2) = gattling_building_on_shot_fired(l1, c1, Some(10), Some(10), 8, 100);
        assert_eq!(l2, GattlingFireLevel::Mean);
        assert_eq!(c2, 2);
        assert!(!f2);

        // Continue to shot 6 → Fast (consecutive 6 > 5).
        let mut level = l2;
        let mut consec = c2;
        for shot in 3..=6 {
            let (nl, nc, entered) =
                gattling_building_on_shot_fired(level, consec, Some(10), Some(10), shot * 4, 1000);
            level = nl;
            consec = nc;
            if shot == 6 {
                assert_eq!(level, GattlingFireLevel::Fast);
                assert!(entered || level == GattlingFireLevel::Fast);
            }
        }
        assert_eq!(level, GattlingFireLevel::Fast);
        assert_eq!(consec, 6);
    }

    #[test]
    fn delay_and_chain_guns_math() {
        assert_eq!(
            gattling_building_delay_frames_for_level(GattlingFireLevel::Base),
            8
        );
        assert_eq!(
            gattling_building_delay_frames_for_level(GattlingFireLevel::Mean),
            4
        );
        assert_eq!(
            gattling_building_delay_frames_for_level(GattlingFireLevel::Fast),
            2
        );

        let ground = gattling_building_ground_weapon(GattlingFireLevel::Base, false);
        assert!((ground.damage - 10.0).abs() < 0.01);
        assert!((ground.range - 225.0).abs() < 0.01);
        assert!(!ground.can_target_air);
        assert!(ground.can_target_ground);

        let air = gattling_building_air_weapon(GattlingFireLevel::Base, false);
        assert!((air.damage - 5.0).abs() < 0.01);
        assert!((air.range - 400.0).abs() < 0.01);
        assert!(air.can_target_air);
        assert!(!air.can_target_ground);

        let chained = gattling_building_ground_weapon(GattlingFireLevel::Base, true);
        assert!((chained.damage - 12.5).abs() < 0.01);

        let mut tags = HashSet::new();
        assert!(!gattling_building_has_chain_guns(&tags));
        tags.insert("Upgrade_ChinaChainGuns".to_string());
        assert!(gattling_building_has_chain_guns(&tags));

        assert_eq!(preferred_gattling_building_slot(false), 0);
        assert_eq!(preferred_gattling_building_slot(true), 1);
    }

    #[test]
    fn patriot_assist_matrix_honesty() {
        assert!((PATRIOT_REQUEST_ASSIST_RANGE - 200.0).abs() < 0.01);
        assert_eq!(PATRIOT_ASSISTING_CLIP_SIZE, 4);
        assert!((PATRIOT_ASSIST_RANGE - 450.0).abs() < 0.01);
        assert!((PATRIOT_ASSIST_DAMAGE - 25.0).abs() < 0.01);
        assert!((LAZR_PATRIOT_ASSIST_DAMAGE - 35.0).abs() < 0.01);
        assert_eq!(PATRIOT_ASSIST_DELAY_FRAMES, 8);
        assert_eq!(PATRIOT_ASSIST_CLIP_RELOAD_FRAMES, 30);

        assert!((patriot_assist_damage_for_template("AmericaPatriotBattery") - 25.0).abs() < 0.01);
        assert!((patriot_assist_damage_for_template("Lazr_AmericaPatriotBattery") - 35.0).abs() < 0.01);
        assert!((patriot_assist_damage_for_template("SupW_AmericaPatriotBattery") - 25.0).abs() < 0.01);
        assert_eq!(
            patriot_assist_weapon_name_for_template("AmericaPatriotBattery"),
            PATRIOT_ASSIST_WEAPON
        );
        assert_eq!(
            patriot_assist_weapon_name_for_template("Lazr_AmericaPatriotBattery"),
            LAZR_PATRIOT_ASSIST_WEAPON
        );
        assert_eq!(
            patriot_assist_weapon_name_for_template("SupW_AmericaPatriotBattery"),
            SUPW_PATRIOT_ASSIST_WEAPON
        );

        assert!(patriots_are_assist_equivalent(
            "AmericaPatriotBattery",
            "USA_Patriot"
        ));
        assert!(patriots_are_assist_equivalent(
            "Lazr_AmericaPatriotBattery",
            "TestLazrPatriot"
        ));
        assert!(!patriots_are_assist_equivalent(
            "AmericaPatriotBattery",
            "Lazr_AmericaPatriotBattery"
        ));
        assert!(!patriots_are_assist_equivalent(
            "AmericaPatriotBattery",
            "SupW_AmericaPatriotBattery"
        ));
        assert!(!patriots_are_assist_equivalent(
            "AmericaPatriotBattery",
            "ChinaGattlingCannon"
        ));

        assert!(is_patriot_free_to_assist(true, true, true, false, false, true));
        assert!(!is_patriot_free_to_assist(
            true, true, true, false, true, true
        )); // mid-assist
        assert!(!is_patriot_free_to_assist(
            true, true, true, false, false, false
        )); // weapon not ready
        assert!(!is_patriot_free_to_assist(
            true, false, true, false, false, true
        )); // not constructed

        assert!(is_within_patriot_request_assist_range(200.0));
        assert!(!is_within_patriot_request_assist_range(201.0));
        assert!(is_within_patriot_assist_weapon_range(450.0));
        assert!(!is_within_patriot_assist_weapon_range(451.0));

        let pending = PendingPatriotAssist::new(
            ObjectId(1),
            ObjectId(2),
            ObjectId(3),
            10,
            "AmericaPatriotBattery",
        );
        assert_eq!(pending.shots_remaining, 4);
        assert_eq!(pending.next_shot_frame, 10);
        assert!((pending.damage() - 25.0).abs() < 0.01);
        assert!(!pending.is_supw());

        // BinaryDataStream laser residual (LaserFromAssisted + LaserToTarget).
        assert_eq!(PATRIOT_BINARY_DATA_STREAM, "PatriotBinaryDataStream");
        assert_eq!(PATRIOT_ASSIST_LASER_LIFETIME_FRAMES, 18);
        assert_eq!(patriot_assist_laser_expires_frame(10), 28);
        // W3DLaserDraw residual honesty params.
        assert_eq!(PATRIOT_LASER_NUM_BEAMS, 1);
        assert!((PATRIOT_LASER_INNER_BEAM_WIDTH - 4.0).abs() < 0.001);
        assert!((PATRIOT_LASER_SCROLL_RATE + 0.25).abs() < 0.001);
        assert_eq!(PATRIOT_LASER_SEGMENTS, 20);
        assert!((PATRIOT_LASER_ARC_HEIGHT - 30.0).abs() < 0.001);
        assert!((PATRIOT_LASER_TILING_SCALAR - 0.25).abs() < 0.001);
        let beams = make_patriot_assist_lasers(
            ObjectId(1),
            ObjectId(2),
            ObjectId(3),
            (0.0, 0.0, 0.0),
            (100.0, 0.0, 0.0),
            (50.0, 0.0, 0.0),
            10,
        );
        assert_eq!(beams[0].kind, PatriotAssistLaserKind::FromAssisted);
        assert_eq!(beams[0].from_id, ObjectId(1));
        assert_eq!(beams[0].to_id, ObjectId(2));
        assert_eq!(beams[0].template_name(), PATRIOT_BINARY_DATA_STREAM);
        assert_eq!(beams[0].num_beams(), 1);
        assert!((beams[0].arc_height() - 30.0).abs() < 0.001);
        assert!((beams[0].inner_beam_width() - 4.0).abs() < 0.001);
        assert_eq!(beams[0].segments(), 20);
        assert!((beams[0].scroll_offset - 0.0).abs() < 0.001);
        assert!(!beams[0].endpoint_tracked);
        assert!(beams[0].is_active_at(10));
        assert!(beams[0].is_active_at(27));
        assert!(!beams[0].is_active_at(28));
        assert_eq!(beams[1].kind, PatriotAssistLaserKind::ToTarget);
        assert_eq!(beams[1].from_id, ObjectId(2));
        assert_eq!(beams[1].to_id, ObjectId(3));
        // LaserUpdate endpoint track residual: move requestor + victim.
        let mut live = beams.to_vec();
        let mut positions = std::collections::HashMap::from([
            (ObjectId(1), (10.0_f32, 0.0, 5.0, true)),
            (ObjectId(2), (100.0_f32, 0.0, 0.0, true)),
            (ObjectId(3), (60.0_f32, 0.0, 0.0, true)),
        ]);
        let moved = track_patriot_assist_laser_endpoints(&mut live, |id| positions.get(&id).copied());
        assert!(moved >= 1, "endpoint residual must track moved parent/target");
        assert!((live[0].from_x - 10.0).abs() < 0.01);
        assert!((live[0].from_z - 5.0).abs() < 0.01);
        assert!((live[1].to_x - 60.0).abs() < 0.01);
        assert!(live[0].endpoint_tracked || live[1].endpoint_tracked);
        // ScrollRate residual advances each track step.
        assert!((live[0].scroll_offset - PATRIOT_LASER_SCROLL_RATE).abs() < 0.001);
        // Dead target freezes last end residual.
        positions.insert(ObjectId(3), (99.0, 0.0, 0.0, false));
        let end_before = (live[1].to_x, live[1].to_y, live[1].to_z);
        track_patriot_assist_laser_endpoints(&mut live, |id| positions.get(&id).copied());
        assert!((live[1].to_x - end_before.0).abs() < 0.01);
        assert!((live[1].to_y - end_before.1).abs() < 0.01);
        assert!((live[1].to_z - end_before.2).abs() < 0.01);
        assert_eq!(expire_patriot_assist_lasers(&mut live, 27), 0);
        assert_eq!(live.len(), 2);
        assert_eq!(expire_patriot_assist_lasers(&mut live, 28), 2);
        assert!(live.is_empty());
    }

    #[test]
    fn stinger_hive_structure_body_matrix_honesty() {
        assert_eq!(STINGER_SPAWN_NUMBER, 3);
        assert!((STINGER_SOLDIER_MAX_HEALTH - 100.0).abs() < 0.01);
        assert_eq!(STINGER_SPAWN_REPLACE_DELAY_FRAMES, 900);

        let (count, hp) = init_stinger_hive_slaves();
        assert_eq!(count, 3);
        assert!((hp - 100.0).abs() < 0.01);
        assert!(stinger_can_fire_with_slaves(3));
        assert!(stinger_can_fire_with_slaves(1));
        assert!(!stinger_can_fire_with_slaves(0));

        // Propagate residual: damages slaves, not structure.
        let (c, h, struct_hp, r) = resolve_hive_structure_damage(
            3,
            100.0,
            1000.0,
            40.0,
            HostHiveDamageClass::PropagateToSlaves,
        );
        assert_eq!(c, 3);
        assert!((h - 60.0).abs() < 0.01);
        assert!((struct_hp - 1000.0).abs() < 0.01);
        assert!((r.slave_damage_applied - 40.0).abs() < 0.01);
        assert_eq!(r.slaves_killed, 0);
        assert!(!r.swallowed);

        // Kill one residual slave with lethal propagate.
        let (c2, h2, struct_hp2, r2) = resolve_hive_structure_damage(
            3,
            40.0,
            1000.0,
            50.0,
            HostHiveDamageClass::PropagateToSlaves,
        );
        assert_eq!(c2, 2);
        assert!((h2 - STINGER_SOLDIER_MAX_HEALTH).abs() < 0.01);
        assert!((struct_hp2 - 1000.0).abs() < 0.01);
        assert_eq!(r2.slaves_killed, 1);

        // Swallow residual when no slaves (SNIPER path).
        let (c3, _, struct_hp3, r3) = resolve_hive_structure_damage(
            0,
            0.0,
            1000.0,
            999.0,
            HostHiveDamageClass::SwallowIfNoSlaves,
        );
        assert_eq!(c3, 0);
        assert!((struct_hp3 - 1000.0).abs() < 0.01);
        assert!(r3.swallowed);
        assert!((r3.structure_damage_applied - 0.0).abs() < 0.01);

        // HitStructure residual damages building even with slaves.
        let (c4, h4, struct_hp4, r4) = resolve_hive_structure_damage(
            3,
            100.0,
            1000.0,
            200.0,
            HostHiveDamageClass::HitStructure,
        );
        assert_eq!(c4, 3);
        assert!((h4 - 100.0).abs() < 0.01);
        assert!((struct_hp4 - 800.0).abs() < 0.01);
        assert!((r4.structure_damage_applied - 200.0).abs() < 0.01);

        // Propagate with no slaves falls through to structure residual.
        let (_, _, struct_hp5, r5) = resolve_hive_structure_damage(
            0,
            0.0,
            500.0,
            100.0,
            HostHiveDamageClass::PropagateToSlaves,
        );
        assert!((struct_hp5 - 400.0).abs() < 0.01);
        assert!(!r5.swallowed);

        assert_eq!(next_stinger_slave_respawn_frame(10, 0), 910);
        assert_eq!(next_stinger_slave_respawn_frame(10, 950), 950);
        assert!(should_respawn_stinger_slave(2, 910, 910));
        assert!(!should_respawn_stinger_slave(3, 910, 910));
        assert!(!should_respawn_stinger_slave(2, 900, 910));
    }

    #[test]
    fn stinger_get_closest_slave_roster_honesty() {
        let roster = init_stinger_hive_slave_roster();
        assert_eq!(roster.len(), 3);
        assert_eq!(count_alive_hive_slaves(&roster), 3);
        assert_eq!(STINGER_SPAWN_TEMPLATE, "GLAInfantryStingerSoldier");
        assert!((STINGER_SPAWN_POINT_RADIUS - 12.0).abs() < 0.001);

        // Offsets form a 120° ring residual.
        let offs = stinger_spawn_point_offsets();
        assert!((offs[0].0 - STINGER_SPAWN_POINT_RADIUS).abs() < 0.01);
        assert!((offs[0].1).abs() < 0.01);

        // Shooter near slave 0 (+radius, 0) → index 0.
        let i0 = get_closest_hive_slave_index(&roster, 0.0, 0.0, 100.0, 0.0);
        assert_eq!(i0, Some(0));
        // Shooter near slave 1 (-half, +y) → index 1.
        let (sx1, sz1) = roster[1].world_xz(0.0, 0.0);
        let i1 = get_closest_hive_slave_index(&roster, 0.0, 0.0, sx1 + 1.0, sz1);
        assert_eq!(i1, Some(1));
        // Shooter near slave 2 → index 2.
        let (sx2, sz2) = roster[2].world_xz(0.0, 0.0);
        let i2 = get_closest_hive_slave_index(&roster, 0.0, 0.0, sx2, sz2 - 1.0);
        assert_eq!(i2, Some(2));

        // Kill slave 0: closest to +x becomes slave 1 or 2 (not 0).
        let mut live = roster;
        live[0].alive = false;
        live[0].hp = 0.0;
        assert_eq!(count_alive_hive_slaves(&live), 2);
        let i = get_closest_hive_slave_index(&live, 0.0, 0.0, 100.0, 0.0);
        assert!(i == Some(1) || i == Some(2));
        assert_ne!(i, Some(0));

        // Damage closest (slave 1) with roster residual.
        let mut slaves = roster;
        let (_, struct_hp, r) = resolve_hive_structure_damage_roster(
            &mut slaves,
            1000.0,
            40.0,
            HostHiveDamageClass::PropagateToSlaves,
            Some((0.0, 0.0, sx1, sz1)),
        );
        assert!((struct_hp - 1000.0).abs() < 0.01);
        assert_eq!(r.closest_slave_index, Some(1));
        assert!((r.slave_damage_applied - 40.0).abs() < 0.01);
        assert!((slaves[1].hp - 60.0).abs() < 0.01);
        assert!((slaves[0].hp - 100.0).abs() < 0.01);
        assert!((slaves[2].hp - 100.0).abs() < 0.01);

        // Kill closest slave 1.
        let (_, _, r2) = resolve_hive_structure_damage_roster(
            &mut slaves,
            1000.0,
            80.0,
            HostHiveDamageClass::PropagateToSlaves,
            Some((0.0, 0.0, sx1, sz1)),
        );
        assert_eq!(r2.slaves_killed, 1);
        assert!(!slaves[1].alive);
        assert_eq!(count_alive_hive_slaves(&slaves), 2);

        // Respawn residual revives first dead slot.
        assert!(respawn_one_hive_slave(&mut slaves));
        assert!(slaves[1].alive);
        assert!((slaves[1].hp - STINGER_SOLDIER_MAX_HEALTH).abs() < 0.01);
        assert_eq!(count_alive_hive_slaves(&slaves), 3);
    }

    #[test]
    fn patriot_laser_arc_segment_honesty() {
        // Cos arc: mid = ArcHeight, ends = 0.
        assert!((patriot_laser_arc_peak_boost(PATRIOT_LASER_ARC_HEIGHT) - 30.0).abs() < 0.001);
        assert!((patriot_laser_arc_z_boost(0.0, 30.0)).abs() < 0.001);
        assert!((patriot_laser_arc_z_boost(1.0, 30.0)).abs() < 0.001);
        assert!((patriot_laser_arc_z_boost(0.5, 30.0) - 30.0).abs() < 0.001);
        // Quarter points still raised but less than peak.
        let q = patriot_laser_arc_z_boost(0.25, 30.0);
        assert!(q > 0.0 && q < 30.0);

        let from = (0.0_f32, 0.0, 10.0);
        let to = (100.0_f32, 0.0, 10.0);
        let mid = sample_patriot_laser_arc_point(from, to, 0.5, PATRIOT_LASER_ARC_HEIGHT);
        assert!((mid.0 - 50.0).abs() < 0.01);
        assert!((mid.2 - (10.0 + 30.0)).abs() < 0.01, "mid Z = base + ArcHeight");

        let (s0, e0) =
            sample_patriot_laser_arc_segment(from, to, 0, PATRIOT_LASER_SEGMENTS, PATRIOT_LASER_ARC_HEIGHT);
        assert!((s0.0 - 0.0).abs() < 0.01);
        assert!(e0.0 > s0.0);
        // Last segment ends near target.
        let (_s_last, e_last) = sample_patriot_laser_arc_segment(
            from,
            to,
            PATRIOT_LASER_SEGMENTS - 1,
            PATRIOT_LASER_SEGMENTS,
            PATRIOT_LASER_ARC_HEIGHT,
        );
        assert!((e_last.0 - 100.0).abs() < 0.5);
        assert!((e_last.2 - 10.0).abs() < 0.5); // end arc boost ~0
    }
}
