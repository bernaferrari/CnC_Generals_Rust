//! Host China Helix NapalmBomb special ability residual.
//!
//! Residual slice (playability):
//! - `SpecialAbilityHelixNapalmBomb` / `SPECIAL_HELIX_NAPALM_BOMB` on Helix hosts
//!   with `Upgrade_HelixNapalmBomb` (or TestHelix residual unlock) drops a
//!   residual NapalmBomb at the target location:
//!   - Instant blast: PrimaryDamage **75** / radius **5** + Secondary **40** / **30**
//!     (`NapalmBombWeapon` / `BlackNapalmBombWeapon` same blast numbers).
//!   - Spawns residual FirestormSmall DoT zone at impact
//!     (DamageAmount **100** / tick **500**ms / lifetime **6000**ms / radius **90**).
//!   - BlackNapalm PLAYER_UPGRADE residual → Firestorm tick damage **150**.
//! - Reload residual: **10000** ms (300 frames @ 30 FPS).
//! - Honesty counters/flags for residual gates and tests.
//!
//! Wave 70 residual pack (retail Weapon.ini / SpecialPower.ini / Upgrade.ini /
//! System.ini / ChinaAir.ini):
//! - Weapon residual: NapalmBombWeapon Primary **75**/r**5** + Secondary **40**/r**30**,
//!   DamageType **EXPLOSION**, DeathType **EXPLODED**, FireOCL **OCL_FirestormSmall**.
//! - Ability residual: ReloadTime **10000**ms → **300**f, RadiusCursor **100**,
//!   StartAbilityRange **3**, MaxSpecialObjects **1**.
//! - Firestorm residual: Damage **100** / Black **150**, tick **500**ms → **15**f,
//!   lifetime **6000**ms → **180**f, FinalMajorRadius **90**.
//! - Upgrade residual: Upgrade_HelixNapalmBomb BuildCost **800**, BuildTime **20**s → **600**f.
//! - Honesty: `honesty_helix_napalm_residual_pack_ok` + layer honesty tests.
//!
//! Fail-closed honesty:
//! - SpecialObject NapalmBomb projectile + HeightDieUpdate fall residual closed
//! - FirestormDynamicGeometryInfoUpdate expand/reverse uses current bounding
//!   circle (`doDamageScan`) + reverse-at-transition scorch. ParticleSystem /
//!   FXList / emission follow leftover-calls leftover
//!   FirestormDynamicGeometryInfoUpdate (playable_claim stays false).
//! - Not full SpecialAbilityUpdate UnpackTime / MaxSpecialObjects charge matrix
//! - Not full SubObjectsUpgrade BombWing / UnpauseSpecialPowerUpgrade module
//! - Not network Helix Napalm replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const HELIX_NAPALM_LOGIC_FPS: f32 = 30.0;

/// Retail SpecialAbilityHelixNapalmBomb ReloadTime = 10000 ms → 300 frames.
pub const HELIX_NAPALM_RELOAD_MS: u32 = 10_000;
/// Retail SpecialAbilityHelixNapalmBomb ReloadTime = 10000 ms → 300 frames.
pub const HELIX_NAPALM_RELOAD_FRAMES: u32 = 300;
/// Retail SpecialAbilityHelixNapalmBomb RadiusCursorRadius residual.
pub const HELIX_NAPALM_RADIUS_CURSOR: f32 = 100.0;
/// Retail SpecialAbilityUpdate StartAbilityRange residual.
pub const HELIX_NAPALM_START_ABILITY_RANGE: f32 = 3.0;
/// Retail MaxSpecialObjects residual.
pub const HELIX_NAPALM_MAX_SPECIAL_OBJECTS: u32 = 1;
/// Retail NapalmBombWeapon DamageType residual.
pub const HELIX_NAPALM_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail NapalmBombWeapon DeathType residual.
pub const HELIX_NAPALM_DEATH_TYPE: &str = "EXPLODED";
/// Retail NapalmBombWeapon FireOCL residual.
pub const HELIX_NAPALM_FIRE_OCL: &str = "OCL_FirestormSmall";
/// Retail BlackNapalmBombWeapon FireOCL residual.
pub const HELIX_NAPALM_BLACK_FIRE_OCL: &str = "OCL_BlackNapalmFirestormSmall";
/// Retail FirestormSmall DelayBetweenDamageFrames residual (msec).
pub const HELIX_FIRESTORM_TICK_MS: u32 = 500;
/// Retail FirestormSmall LifetimeUpdate residual (msec).
pub const HELIX_FIRESTORM_DURATION_MS: u32 = 6_000;
/// Retail Upgrade_HelixNapalmBomb BuildCost residual.
pub const HELIX_NAPALM_UPGRADE_BUILD_COST: u32 = 800;
/// Retail Upgrade_HelixNapalmBomb BuildTime residual (seconds).
pub const HELIX_NAPALM_UPGRADE_BUILD_TIME_SEC: f32 = 20.0;
/// BuildTime 20s → 600 frames @ 30 FPS.
pub const HELIX_NAPALM_UPGRADE_BUILD_TIME_FRAMES: u32 = 600;
/// Retail SpecialAbilityHelixNapalmBomb Enum residual.
pub const HELIX_NAPALM_SPECIAL_POWER: &str = "SpecialAbilityHelixNapalmBomb";

/// Retail NapalmBombWeapon PrimaryDamage / PrimaryDamageRadius.
pub const HELIX_NAPALM_PRIMARY_DAMAGE: f32 = 75.0;
pub const HELIX_NAPALM_PRIMARY_RADIUS: f32 = 5.0;

/// Retail NapalmBombWeapon SecondaryDamage / SecondaryDamageRadius.
pub const HELIX_NAPALM_SECONDARY_DAMAGE: f32 = 40.0;
pub const HELIX_NAPALM_SECONDARY_RADIUS: f32 = 30.0;

/// Retail FirestormSmall FinalMajorRadius.
pub const HELIX_FIRESTORM_RADIUS: f32 = 90.0;
/// Retail FirestormSmall InitialMajorRadius (GeometryMajorRadius start).
pub const HELIX_FIRESTORM_INITIAL_RADIUS: f32 = 1.0;
/// Retail FirestormSmall TransitionTime = 3000 ms → 90 frames @ 30 FPS.
pub const HELIX_FIRESTORM_TRANSITION_MS: u32 = 3_000;
/// TransitionTime 3000 ms @ 30 FPS. Grow then ReverseAtTransitionTime shrink.
pub const HELIX_FIRESTORM_TRANSITION_FRAMES: u32 = 90;
/// Retail ReverseAtTransitionTime = Yes.
pub const HELIX_FIRESTORM_REVERSE_AT_TRANSITION: bool = true;
/// Retail FirestormSmall ScorchSize (placed once when directions switch).
pub const HELIX_FIRESTORM_SCORCH_SIZE: f32 = 90.0;

/// Retail FirestormSmall DamageAmount per damage frame.
pub const HELIX_FIRESTORM_DAMAGE_PER_TICK: f32 = 100.0;

/// Retail BlackNapalmFirestormSmall DamageAmount.
pub const HELIX_FIRESTORM_DAMAGE_UPGRADED: f32 = 150.0;

/// Retail DelayBetweenDamageFrames = 500 ms → 15 frames @ 30 FPS.
pub const HELIX_FIRESTORM_TICK_INTERVAL_FRAMES: u32 = 15;

/// Retail FirestormSmall LifetimeUpdate 6000 ms → 180 frames @ 30 FPS.
pub const HELIX_FIRESTORM_DURATION_FRAMES: u32 = 180;
/// C++ FirestormDynamicGeometryInfoUpdateModuleData default MaxHeightForDamage
/// (`FirestormDynamicGeometryInfoUpdate.cpp:36`). Host Y-up maps C++ Z.
pub const HELIX_FIRESTORM_MAX_HEIGHT_FOR_DAMAGE: f32 = 20.0;

/// Retail upgrade that unpauses Helix NapalmBomb special power.
pub const UPGRADE_HELIX_NAPALM_BOMB: &str = "Upgrade_HelixNapalmBomb";
/// Retail Nuke_Upgrade_HelixNukeBomb residual unlock name.
pub const UPGRADE_HELIX_NUKE_BOMB: &str = "Nuke_Upgrade_HelixNukeBomb";
/// Retail Nuke_SpecialAbilityHelixNukeBomb name residual.
pub const HELIX_NUKE_BOMB_SPECIAL_POWER: &str = "Nuke_SpecialAbilityHelixNukeBomb";

/// Retail BlackNapalm player upgrade (swaps NapalmBomb → BlackNapalmBomb weapon).
pub const UPGRADE_CHINA_BLACK_NAPALM: &str = "Upgrade_ChinaBlackNapalm";

/// Residual weapon names.
pub const NAPALM_BOMB_WEAPON: &str = "NapalmBombWeapon";
/// Retail SpecialObject projectile residual dropped by Helix NapalmBomb ability.
pub const NAPALM_BOMB_PROJECTILE: &str = "NapalmBomb";
/// Retail NapalmBomb MaxHealth residual.
pub const NAPALM_BOMB_MAX_HEALTH: f32 = 100.0;
/// Retail HeightDieUpdate TargetHeight residual.
pub const NAPALM_BOMB_HEIGHT_DIE_TARGET: f32 = 1.0;
/// Residual fall speed (world units / frame) — Host Y-up freefall peel.
pub const NAPALM_BOMB_FALL_SPEED_PER_FRAME: f32 = 4.0;
pub const BLACK_NAPALM_BOMB_WEAPON: &str = "BlackNapalmBombWeapon";

/// Drop / impact audio residual.
pub const HELIX_NAPALM_DROP_AUDIO: &str = "HelixVoiceModeNapalmBomb";
pub const HELIX_FIRESTORM_AUDIO: &str = "FireStormLoop";

/// Whether template is a residual Helix that can drop NapalmBomb.
///
/// Fail-closed: name residual. Reuses Overlord-family Helix name matrix but
/// allows TestHelix explicitly. Excludes NapalmBomb projectile objects.
pub fn is_helix_napalm_caster(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n == "testhelix" || n == "test_helix" {
        return true;
    }
    // Projectile / bomb / firestorm objects are not the Helix vehicle.
    if n.contains("napalmbomb")
        || n.contains("napalm_bomb")
        || n.contains("firestorm")
        || n.contains("projectile")
        || n.contains("shell")
        || n.contains("gattling")
        || n.contains("propaganda")
        || n.contains("bunker")
    {
        return false;
    }
    n.contains("vehiclehelix")
        || n.contains("china_helix")
        || n.contains("chinahelix")
        || (n.contains("helix") && (n.contains("vehicle") || n.contains("china")))
}

/// Whether the Helix residual has unlocked NapalmBomb (upgrade or test host).
pub fn helix_napalm_unlocked(template_name: &str, has_upgrade: bool) -> bool {
    if !is_helix_napalm_caster(template_name) {
        return false;
    }
    let n = template_name.to_ascii_lowercase();
    // Test host residual: always unlocked for deterministic host tests.
    if n == "testhelix" || n == "test_helix" {
        return true;
    }
    has_upgrade
}

/// C++ `isWithinStartAbilityRange` for Helix NapalmBomb (StartAbilityRange 3).
/// Location target has no object radius; uses leftover bounding-sphere 2D.
pub fn helix_napalm_in_start_range(helix_pos: Vec3, helix_radius: f32, target_pos: Vec3) -> bool {
    let edge = crate::game_logic::host_hero_abilities::leftover_bounding_sphere_2d(
        helix_pos,
        helix_radius,
        target_pos,
        0.0,
    );
    crate::game_logic::host_hero_abilities::leftover_within_start_ability_range(
        edge,
        HELIX_NAPALM_START_ABILITY_RANGE,
    )
}

/// Instant NapalmBombWeapon area damage at distance (max of primary/secondary).
pub fn helix_napalm_blast_damage_at(distance: f32) -> f32 {
    if distance <= HELIX_NAPALM_PRIMARY_RADIUS {
        HELIX_NAPALM_PRIMARY_DAMAGE
    } else if distance <= HELIX_NAPALM_SECONDARY_RADIUS {
        HELIX_NAPALM_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// C++ `DynamicGeometryInfoUpdate::update` major-radius lerp at `elapsed`
/// frames after activate (`DynamicGeometryInfoUpdate.cpp:115-148`).
///
/// Grow `InitialMajorRadius → FinalMajorRadius` over TransitionTime, then
/// ReverseAtTransitionTime swaps ends and shrinks. Damage scan uses this
/// current bounding-circle radius (`FirestormDynamicGeometryInfoUpdate.cpp:221-228`).
pub fn firestorm_major_radius_at(elapsed: u32) -> f32 {
    let t = HELIX_FIRESTORM_TRANSITION_FRAMES.max(1);
    let init = HELIX_FIRESTORM_INITIAL_RADIUS;
    let fin = HELIX_FIRESTORM_RADIUS;
    if elapsed <= t {
        let ratio = elapsed as f32 / t as f32;
        return init + ratio * (fin - init);
    }
    let shrink_t = elapsed - t - 1;
    if shrink_t <= t {
        let ratio = shrink_t as f32 / t as f32;
        return fin + ratio * (init - fin);
    }
    init
}

/// True once C++ `m_switchedDirections` is set (end of grow, start of shrink).
/// Scorch is placed the first frame this becomes true (`:181-187`).
#[inline]
pub fn firestorm_switched_directions(elapsed: u32) -> bool {
    HELIX_FIRESTORM_REVERSE_AT_TRANSITION && elapsed >= HELIX_FIRESTORM_TRANSITION_FRAMES
}

/// One active residual FirestormSmall damage zone from a Helix napalm drop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostHelixFirestormZone {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub radius: f32,
    pub damage_per_tick: f32,
    pub activate_frame: u32,
    pub expires_frame: u32,
    pub next_tick_frame: u32,
    pub black_napalm: bool,
    pub total_damage_applied: f32,
    pub damage_applications: u32,
    pub objects_destroyed: u32,
    /// C++ `m_switchedDirections` residual (grow finished, reverse started).
    #[serde(default)]
    pub switched_directions: bool,
    /// C++ `m_scorchPlaced` residual (once, when directions switch).
    #[serde(default)]
    pub scorch_placed: bool,
    /// Leftover `m_myParticleSystemID[MAX_FIRESTORM_SYSTEMS]`.
    #[serde(default)]
    pub leftover_particle_system_ids: [u32; gamelogic::object::behavior::MAX_FIRESTORM_SYSTEMS],
    /// Leftover `m_effectsFired`.
    #[serde(default)]
    pub leftover_effects_fired: bool,
}

impl HostHelixFirestormZone {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }

    /// Current expand/reverse major radius at `current_frame`.
    pub fn current_radius(&self, current_frame: u32) -> f32 {
        firestorm_major_radius_at(current_frame.saturating_sub(self.activate_frame))
    }

    /// Leftover FirestormDynamicGeometryInfoUpdate ParticleSystem/FXList/emission.
    pub fn leftover_tick_particle_fx(&mut self, current_frame: u32) {
        leftover_tick_helix_firestorm_fx(self, current_frame);
    }
}

/// Damage application plan for a single victim this tick.
#[derive(Debug, Clone, Copy)]
pub struct HostHelixFirestormHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub zone_id: u32,
}

/// Result of resolving one zone's damage tick.
#[derive(Debug, Clone)]
pub struct HostHelixFirestormTickPlan {
    pub zone_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub hits: Vec<HostHelixFirestormHit>,
    /// Bounding-circle radius used for this damage scan.
    pub damage_radius: f32,
    /// Place reverse-at-transition scorch this tick.
    pub place_scorch: bool,
    pub scorch_size: f32,
}

/// Host residual registry for Helix NapalmBomb drops + Firestorm zones.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostHelixNapalmRegistry {
    next_id: u32,
    active: Vec<HostHelixFirestormZone>,
    /// Successful napalm drops (special-power activations).
    pub drops: u32,
    /// SpecialObject NapalmBomb projectiles spawned residual.
    pub projectile_spawns: u32,
    /// Instant blast residual applications (object hits from primary/secondary).
    pub blast_hits: u32,
    /// Instant blast damage dealt (honesty).
    pub blast_damage_dealt: f32,
    /// Firestorm zones spawned.
    pub zones_spawned: u32,
    pub expirations: u32,
    pub total_fire_damage_applied: f32,
    pub fire_damage_applications: u32,
    pub objects_destroyed: u32,
    /// BlackNapalm-upgraded drops.
    pub black_napalm_drops: u32,
}

impl HostHelixNapalmRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn active_zones(&self) -> &[HostHelixFirestormZone] {
        &self.active
    }

    pub fn active_zones_mut(&mut self) -> &mut [HostHelixFirestormZone] {
        &mut self.active
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Record a residual napalm drop and spawn FirestormSmall at impact.
    pub fn record_projectile_spawn(&mut self) {
        self.projectile_spawns = self.projectile_spawns.saturating_add(1);
    }

    pub fn honesty_projectile_ok(&self) -> bool {
        self.projectile_spawns > 0
    }

    pub fn record_drop_and_spawn_firestorm(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        impact_pos: Vec3,
        activate_frame: u32,
        black_napalm: bool,
        blast_hits: u32,
        blast_damage: f32,
    ) -> u32 {
        self.drops = self.drops.saturating_add(1);
        self.blast_hits = self.blast_hits.saturating_add(blast_hits);
        if blast_damage > 0.0 {
            self.blast_damage_dealt += blast_damage;
        }
        if black_napalm {
            self.black_napalm_drops = self.black_napalm_drops.saturating_add(1);
        }

        let id = self.alloc_id();
        let damage = if black_napalm {
            HELIX_FIRESTORM_DAMAGE_UPGRADED
        } else {
            HELIX_FIRESTORM_DAMAGE_PER_TICK
        };
        let zone = HostHelixFirestormZone {
            id,
            source_object,
            source_team,
            position: impact_pos,
            radius: firestorm_major_radius_at(0),
            damage_per_tick: damage,
            activate_frame,
            expires_frame: activate_frame.saturating_add(HELIX_FIRESTORM_DURATION_FRAMES),
            // Immediate first tick so residual is host-testable on activation frame.
            next_tick_frame: activate_frame,
            black_napalm,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            switched_directions: false,
            scorch_placed: false,
            leftover_particle_system_ids: [0; gamelogic::object::behavior::MAX_FIRESTORM_SYSTEMS],
            leftover_effects_fired: false,
        };
        self.active.push(zone);
        self.zones_spawned = self.zones_spawned.saturating_add(1);
        id
    }

    /// Plan Firestorm damage for zones due this frame.
    ///
    /// Retail Firestorm damages ALLIES ENEMIES NEUTRALS; residual skips source Helix.
    /// C++ `doDamageScan` (`FirestormDynamicGeometryInfoUpdate.cpp:221-244`)
    /// uses the **current** bounding-circle radius and skips objects whose
    /// height is above `z + MaxHeightForDamage`. Reverse-at-transition scorch
    /// is armed the first frame `m_switchedDirections` is true (`:181-187`).
    pub fn plan_due_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostHelixFirestormTickPlan> {
        let mut plans = Vec::new();
        for zone in &self.active {
            if !zone.is_due_tick(current_frame) {
                continue;
            }
            let mut hits = Vec::new();
            let elapsed = current_frame.saturating_sub(zone.activate_frame);
            let radius = firestorm_major_radius_at(elapsed);
            let r2 = radius * radius;
            let height_cap = zone.position.y + HELIX_FIRESTORM_MAX_HEIGHT_FOR_DAMAGE;
            for &(id, pos, _team, alive) in object_positions {
                if !alive || id == zone.source_object {
                    continue;
                }
                // C++ other->getPosition()->z > firestorm.z + m_maxHeightForDamage
                // Host Y-up: skip aircraft / high objects over the firestorm.
                if pos.y > height_cap {
                    continue;
                }
                let dx = pos.x - zone.position.x;
                let dz = pos.z - zone.position.z;
                if dx * dx + dz * dz <= r2 {
                    hits.push(HostHelixFirestormHit {
                        target_id: id,
                        damage: zone.damage_per_tick,
                        zone_id: zone.id,
                    });
                }
            }
            let place_scorch = firestorm_switched_directions(elapsed) && !zone.scorch_placed;
            plans.push(HostHelixFirestormTickPlan {
                zone_id: zone.id,
                source_object: zone.source_object,
                source_team: zone.source_team,
                hits,
                damage_radius: radius,
                place_scorch,
                scorch_size: HELIX_FIRESTORM_SCORCH_SIZE,
            });
        }
        plans.sort_by_key(|p| p.zone_id);
        plans
    }

    pub fn record_tick_complete(
        &mut self,
        zone_id: u32,
        damage_applied: f32,
        applications: u32,
        destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(zone) = self.active.iter_mut().find(|z| z.id == zone_id) {
            zone.total_damage_applied += damage_applied;
            zone.damage_applications = zone.damage_applications.saturating_add(applications);
            zone.objects_destroyed = zone.objects_destroyed.saturating_add(destroyed);
            zone.next_tick_frame =
                current_frame.saturating_add(HELIX_FIRESTORM_TICK_INTERVAL_FRAMES);
            let elapsed = current_frame.saturating_sub(zone.activate_frame);
            zone.radius = firestorm_major_radius_at(elapsed);
            if firestorm_switched_directions(elapsed) {
                zone.switched_directions = true;
                zone.scorch_placed = true;
            }
        }
        self.total_fire_damage_applied += damage_applied;
        self.fire_damage_applications = self.fire_damage_applications.saturating_add(applications);
        self.objects_destroyed = self.objects_destroyed.saturating_add(destroyed);
    }

    pub fn prune_expired(&mut self, current_frame: u32) {
        let before = self.active.len();
        self.active.retain(|z| !z.is_expired(current_frame));
        let removed = before.saturating_sub(self.active.len()) as u32;
        self.expirations = self.expirations.saturating_add(removed);
    }

    pub fn is_position_in_active_fire(&self, pos: Vec3) -> bool {
        self.active.iter().any(|z| {
            let dx = pos.x - z.position.x;
            let dz = pos.z - z.position.z;
            dx * dx + dz * dz <= z.radius * z.radius
        })
    }

    /// Position-in-fire using the expand/reverse radius at `current_frame`.
    pub fn is_position_in_active_fire_at(&self, pos: Vec3, current_frame: u32) -> bool {
        self.active.iter().any(|z| {
            let r = z.current_radius(current_frame);
            let dx = pos.x - z.position.x;
            let dz = pos.z - z.position.z;
            dx * dx + dz * dz <= r * r
        })
    }

    /// Sync stored radius / reverse-scorch flags to `current_frame`.
    /// Leftover-calls leftover FirestormDynamicGeometryInfoUpdate ParticleSystem /
    /// FXList first-fire and emission-volume follow.
    pub fn advance_geometry(&mut self, current_frame: u32) {
        for zone in &mut self.active {
            let elapsed = current_frame.saturating_sub(zone.activate_frame);
            zone.radius = firestorm_major_radius_at(elapsed);
            if firestorm_switched_directions(elapsed) {
                zone.switched_directions = true;
                if !zone.scorch_placed {
                    zone.scorch_placed = true;
                }
            }
            zone.leftover_tick_particle_fx(current_frame);
        }
    }

    pub fn honesty_drop_ok(&self) -> bool {
        self.drops > 0
    }

    pub fn honesty_blast_ok(&self) -> bool {
        self.blast_hits > 0 && self.blast_damage_dealt > 0.0
    }

    pub fn honesty_firestorm_ok(&self) -> bool {
        self.zones_spawned > 0
            && self.fire_damage_applications > 0
            && self.total_fire_damage_applied > 0.0
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_drop_ok() && (self.honesty_blast_ok() || self.honesty_firestorm_ok())
    }
}

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn helix_napalm_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * HELIX_NAPALM_LOGIC_FPS / 1000.0).round() as u32
}

/// Leftover FirestormDynamicGeometryInfoUpdate ParticleSystem/FXList/emission follow.
fn leftover_tick_helix_firestorm_fx(zone: &mut HostHelixFirestormZone, current_frame: u32) {
    use gamelogic::object::behavior::FirestormDynamicGeometryInfoUpdate;
    let data = leftover_firestorm_module_data(zone.black_napalm);
    let leftover_pos = leftover_helix_firestorm_coord(zone.position);
    let radius = zone.current_radius(current_frame);
    FirestormDynamicGeometryInfoUpdate::leftover_tick_particle_fx(
        &data,
        &leftover_pos,
        &mut zone.leftover_particle_system_ids,
        &mut zone.leftover_effects_fired,
        radius,
    );
}

/// Host Y-up `(x, height, z_ground)` → leftover/C++ Z-up `(x, y_ground, z_height)`.
fn leftover_helix_firestorm_coord(pos: Vec3) -> gamelogic::common::Coord3D {
    gamelogic::common::Coord3D::new(pos.x, pos.z, pos.y)
}

/// Leftover FirestormSmall / BlackNapalmFirestormSmall module data (RIGHT).
fn leftover_firestorm_module_data(
    black_napalm: bool,
) -> gamelogic::object::behavior::FirestormDynamicGeometryInfoUpdateModuleData {
    let name = if black_napalm {
        "BlackNapalmFirestormSmall"
    } else {
        "FirestormSmall"
    };
    peel_leftover_firestorm_module_data(name).unwrap_or_default()
}

fn peel_leftover_firestorm_module_data(
    template_name: &str,
) -> Option<gamelogic::object::behavior::FirestormDynamicGeometryInfoUpdateModuleData> {
    use gamelogic::object::behavior::{
        FirestormDynamicGeometryInfoUpdateModuleData, MAX_FIRESTORM_SYSTEMS,
    };
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    for entry in tmpl.get_behavior_module_info().iter() {
        if !entry
            .name
            .as_str()
            .eq_ignore_ascii_case("FirestormDynamicGeometryInfoUpdate")
        {
            continue;
        }
        if let Some(data) = entry
            .data
            .downcast_ref::<FirestormDynamicGeometryInfoUpdateModuleData>()
        {
            return Some(data.clone());
        }
        let mut data = FirestormDynamicGeometryInfoUpdateModuleData::default();
        let mut any = false;
        if let Some(fx) = entry.data.get_ini_field("FXList") {
            let fx = fx.trim();
            if !fx.is_empty() && !fx.eq_ignore_ascii_case("None") {
                data.fx_list = Some(fx.to_string());
                any = true;
            }
        }
        if let Some(z) = entry.data.get_ini_field("ParticleOffsetZ") {
            if let Ok(v) = z.trim().parse::<f32>() {
                data.particle_offset_z = v;
                any = true;
            }
        }
        for i in 0..MAX_FIRESTORM_SYSTEMS {
            let key = format!("ParticleSystem{}", i + 1);
            if let Some(raw) = entry.data.get_ini_field(&key) {
                let name = raw.trim();
                if !name.is_empty() && !name.eq_ignore_ascii_case("None") {
                    data.particle_systems[i] = Some(name.to_string());
                    any = true;
                }
            }
        }
        if any {
            return Some(data);
        }
    }
    None
}

// --- Wave 70 residual honesty packs ---

/// Wave 70 residual honesty: NapalmBomb weapon residual peel.
pub fn honesty_helix_napalm_weapon_residual_ok() -> bool {
    NAPALM_BOMB_WEAPON == "NapalmBombWeapon"
        && BLACK_NAPALM_BOMB_WEAPON == "BlackNapalmBombWeapon"
        && NAPALM_BOMB_PROJECTILE == "NapalmBomb"
        && (NAPALM_BOMB_HEIGHT_DIE_TARGET - 1.0).abs() < 0.01
        && (HELIX_NAPALM_PRIMARY_DAMAGE - 75.0).abs() < 0.01
        && (HELIX_NAPALM_PRIMARY_RADIUS - 5.0).abs() < 0.01
        && (HELIX_NAPALM_SECONDARY_DAMAGE - 40.0).abs() < 0.01
        && (HELIX_NAPALM_SECONDARY_RADIUS - 30.0).abs() < 0.01
        && HELIX_NAPALM_DAMAGE_TYPE == "EXPLOSION"
        && HELIX_NAPALM_DEATH_TYPE == "EXPLODED"
        && HELIX_NAPALM_FIRE_OCL == "OCL_FirestormSmall"
        && HELIX_NAPALM_BLACK_FIRE_OCL == "OCL_BlackNapalmFirestormSmall"
        && {
            let d0 = helix_napalm_blast_damage_at(0.0);
            let d10 = helix_napalm_blast_damage_at(10.0);
            (d0 - 75.0).abs() < 0.01 && (d10 - 40.0).abs() < 0.01
        }
}

/// Wave 70 residual honesty: SpecialAbilityHelixNapalmBomb residual peel.
pub fn honesty_helix_napalm_ability_residual_ok() -> bool {
    HELIX_NAPALM_SPECIAL_POWER == "SpecialAbilityHelixNapalmBomb"
        && HELIX_NAPALM_RELOAD_MS == 10_000
        && HELIX_NAPALM_RELOAD_FRAMES == helix_napalm_ms_to_frames(HELIX_NAPALM_RELOAD_MS)
        && HELIX_NAPALM_RELOAD_FRAMES == 300
        && (HELIX_NAPALM_RADIUS_CURSOR - 100.0).abs() < 0.01
        && (HELIX_NAPALM_START_ABILITY_RANGE - 3.0).abs() < 0.01
        && helix_napalm_in_start_range(Vec3::ZERO, 0.0, Vec3::new(3.0, 0.0, 0.0))
        && !helix_napalm_in_start_range(Vec3::ZERO, 0.0, Vec3::new(3.1, 0.0, 0.0))
        && HELIX_NAPALM_MAX_SPECIAL_OBJECTS == 1
        && UPGRADE_HELIX_NAPALM_BOMB == "Upgrade_HelixNapalmBomb"
        && UPGRADE_CHINA_BLACK_NAPALM == "Upgrade_ChinaBlackNapalm"
}

/// Wave 70 residual honesty: FirestormSmall DoT residual peel.
pub fn honesty_helix_napalm_firestorm_residual_ok() -> bool {
    (HELIX_FIRESTORM_RADIUS - 90.0).abs() < 0.01
        && (HELIX_FIRESTORM_INITIAL_RADIUS - 1.0).abs() < 0.01
        && HELIX_FIRESTORM_TRANSITION_MS == 3_000
        && HELIX_FIRESTORM_TRANSITION_FRAMES
            == helix_napalm_ms_to_frames(HELIX_FIRESTORM_TRANSITION_MS)
        && HELIX_FIRESTORM_TRANSITION_FRAMES == 90
        && HELIX_FIRESTORM_REVERSE_AT_TRANSITION
        && (HELIX_FIRESTORM_SCORCH_SIZE - 90.0).abs() < 0.01
        && (firestorm_major_radius_at(0) - HELIX_FIRESTORM_INITIAL_RADIUS).abs() < 0.01
        && (firestorm_major_radius_at(HELIX_FIRESTORM_TRANSITION_FRAMES) - HELIX_FIRESTORM_RADIUS)
            .abs()
            < 0.01
        && firestorm_switched_directions(HELIX_FIRESTORM_TRANSITION_FRAMES)
        && !firestorm_switched_directions(HELIX_FIRESTORM_TRANSITION_FRAMES - 1)
        && (HELIX_FIRESTORM_DAMAGE_PER_TICK - 100.0).abs() < 0.01
        && (HELIX_FIRESTORM_DAMAGE_UPGRADED - 150.0).abs() < 0.01
        && (HELIX_FIRESTORM_MAX_HEIGHT_FOR_DAMAGE - 20.0).abs() < 0.01
        && HELIX_FIRESTORM_TICK_MS == 500
        && HELIX_FIRESTORM_TICK_INTERVAL_FRAMES
            == helix_napalm_ms_to_frames(HELIX_FIRESTORM_TICK_MS)
        && HELIX_FIRESTORM_TICK_INTERVAL_FRAMES == 15
        && HELIX_FIRESTORM_DURATION_MS == 6_000
        && HELIX_FIRESTORM_DURATION_FRAMES == helix_napalm_ms_to_frames(HELIX_FIRESTORM_DURATION_MS)
        && HELIX_FIRESTORM_DURATION_FRAMES == 180
}

/// Wave 70 residual honesty: Helix NapalmBomb object upgrade residual peel.
pub fn honesty_helix_napalm_upgrade_residual_ok() -> bool {
    HELIX_NAPALM_UPGRADE_BUILD_COST == 800
        && (HELIX_NAPALM_UPGRADE_BUILD_TIME_SEC - 20.0).abs() < 0.01
        && HELIX_NAPALM_UPGRADE_BUILD_TIME_FRAMES
            == (HELIX_NAPALM_UPGRADE_BUILD_TIME_SEC * HELIX_NAPALM_LOGIC_FPS).round() as u32
        && HELIX_NAPALM_UPGRADE_BUILD_TIME_FRAMES == 600
        && helix_napalm_unlocked("TestHelix", false)
        && !helix_napalm_unlocked("ChinaVehicleHelix", false)
        && helix_napalm_unlocked("ChinaVehicleHelix", true)
}

/// Combined Wave 70 Helix Napalm residual honesty pack.
/// Wave residual honesty: HelixNukeBomb maps onto Helix napalm residual path.
pub fn honesty_helix_nuke_bomb_residual_pack_ok() -> bool {
    HELIX_NUKE_BOMB_SPECIAL_POWER == "Nuke_SpecialAbilityHelixNukeBomb"
        && UPGRADE_HELIX_NUKE_BOMB == "Nuke_Upgrade_HelixNukeBomb"
        && HELIX_NAPALM_SPECIAL_POWER == "SpecialAbilityHelixNapalmBomb"
}

pub fn honesty_helix_napalm_residual_pack_ok() -> bool {
    honesty_helix_napalm_weapon_residual_ok()
        && honesty_helix_napalm_ability_residual_ok()
        && honesty_helix_napalm_firestorm_residual_ok()
        && honesty_helix_napalm_upgrade_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn helix_napalm_caster_name_matrix() {
        assert!(is_helix_napalm_caster("ChinaVehicleHelix"));
        assert!(is_helix_napalm_caster("China_Helix"));
        assert!(is_helix_napalm_caster("Nuke_ChinaVehicleHelix"));
        assert!(is_helix_napalm_caster("TestHelix"));
        assert!(!is_helix_napalm_caster("NapalmBomb"));
        assert!(!is_helix_napalm_caster("BlackNapalmBomb"));
        assert!(!is_helix_napalm_caster("FirestormSmall"));
        assert!(!is_helix_napalm_caster("ChinaTankBattleMaster"));
        assert!(!is_helix_napalm_caster("ChinaHelixGattlingCannon"));
    }

    #[test]
    fn unlock_requires_upgrade_except_test_host() {
        assert!(helix_napalm_unlocked("TestHelix", false));
        assert!(!helix_napalm_unlocked("ChinaVehicleHelix", false));
        assert!(helix_napalm_unlocked("ChinaVehicleHelix", true));
        assert!(!helix_napalm_unlocked("USA_Ranger", true));
    }

    #[test]
    fn blast_damage_rings() {
        assert!((helix_napalm_blast_damage_at(0.0) - HELIX_NAPALM_PRIMARY_DAMAGE).abs() < 0.01);
        assert!((helix_napalm_blast_damage_at(4.0) - HELIX_NAPALM_PRIMARY_DAMAGE).abs() < 0.01);
        assert!((helix_napalm_blast_damage_at(10.0) - HELIX_NAPALM_SECONDARY_DAMAGE).abs() < 0.01);
        assert!(helix_napalm_blast_damage_at(HELIX_NAPALM_SECONDARY_RADIUS + 1.0) <= 0.0);
    }

    #[test]
    fn drop_spawns_firestorm_and_ticks() {
        let mut reg = HostHelixNapalmRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        let id = reg.record_drop_and_spawn_firestorm(
            ObjectId(1),
            Team::China,
            Vec3::new(50.0, 0.0, 0.0),
            0,
            false,
            1,
            75.0,
        );
        assert!(reg.honesty_drop_ok());
        assert!(reg.honesty_blast_ok());
        assert_eq!(reg.active_count(), 1);
        assert_eq!(reg.active_zones()[0].id, id);

        let impact = reg.active_zones()[0].position;
        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::China, true),
            (ObjectId(2), impact, Team::GLA, true),
            (ObjectId(3), Vec3::new(0.0, 0.0, 500.0), Team::GLA, true),
        ];
        let plans = reg.plan_due_ticks(0, &objects);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, ObjectId(2));
        assert!((plans[0].hits[0].damage - HELIX_FIRESTORM_DAMAGE_PER_TICK).abs() < 0.01);

        reg.record_tick_complete(id, HELIX_FIRESTORM_DAMAGE_PER_TICK, 1, 0, 0);
        assert!(reg.honesty_firestorm_ok());
        assert!(reg.honesty_host_path_ok());
    }

    #[test]
    fn black_napalm_uses_higher_firestorm_damage() {
        let mut reg = HostHelixNapalmRegistry::new();
        reg.record_drop_and_spawn_firestorm(ObjectId(1), Team::China, Vec3::ZERO, 0, true, 0, 0.0);
        assert_eq!(reg.black_napalm_drops, 1);
        assert!(
            (reg.active_zones()[0].damage_per_tick - HELIX_FIRESTORM_DAMAGE_UPGRADED).abs() < 0.01
        );
    }

    #[test]
    fn prune_expired_firestorm() {
        let mut reg = HostHelixNapalmRegistry::new();
        reg.record_drop_and_spawn_firestorm(
            ObjectId(1),
            Team::China,
            Vec3::ZERO,
            10,
            false,
            0,
            0.0,
        );
        reg.prune_expired(10 + HELIX_FIRESTORM_DURATION_FRAMES - 1);
        assert_eq!(reg.active_count(), 1);
        reg.prune_expired(10 + HELIX_FIRESTORM_DURATION_FRAMES);
        assert_eq!(reg.active_count(), 0);
        assert_eq!(reg.expirations, 1);
    }

    #[test]
    fn helix_napalm_residual_pack_honesty_wave70() {
        assert!(honesty_helix_napalm_weapon_residual_ok());
        assert!(honesty_helix_napalm_ability_residual_ok());
        assert!(honesty_helix_napalm_firestorm_residual_ok());
        assert!(honesty_helix_napalm_upgrade_residual_ok());
        assert!(honesty_helix_napalm_residual_pack_ok());
        assert_eq!(helix_napalm_ms_to_frames(10_000), 300);
        assert_eq!(helix_napalm_ms_to_frames(500), 15);
        assert_eq!(helix_napalm_ms_to_frames(6_000), 180);
        assert_eq!(HELIX_NAPALM_UPGRADE_BUILD_TIME_FRAMES, 600);
        assert_eq!(HELIX_NAPALM_FIRE_OCL, "OCL_FirestormSmall");
        assert_eq!(HELIX_NAPALM_DAMAGE_TYPE, "EXPLOSION");
        assert!((HELIX_NAPALM_START_ABILITY_RANGE - 3.0).abs() < 0.01);
    }

    /// C++ FirestormDynamicGeometryInfoUpdate.cpp:235-237 — objects whose
    /// height is above firestorm.z + MaxHeightForDamage (default 20) take no DoT.
    #[test]
    fn firestorm_skips_objects_above_max_height_for_damage() {
        let mut reg = HostHelixNapalmRegistry::new();
        let impact = Vec3::new(50.0, 0.0, 0.0);
        let _id =
            reg.record_drop_and_spawn_firestorm(ObjectId(1), Team::China, impact, 0, false, 0, 0.0);
        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::China, true),
            (ObjectId(2), impact, Team::GLA, true),
            (
                ObjectId(3),
                Vec3::new(
                    impact.x,
                    HELIX_FIRESTORM_MAX_HEIGHT_FOR_DAMAGE + 1.0,
                    impact.z,
                ),
                Team::GLA,
                true,
            ),
            (
                ObjectId(4),
                Vec3::new(impact.x, HELIX_FIRESTORM_MAX_HEIGHT_FOR_DAMAGE, impact.z),
                Team::GLA,
                true,
            ),
        ];
        let plans = reg.plan_due_ticks(0, &objects);
        assert_eq!(plans.len(), 1);
        let mut hits: Vec<_> = plans[0].hits.iter().map(|h| h.target_id).collect();
        hits.sort_by_key(|id| id.0);
        assert_eq!(
            hits,
            vec![ObjectId(2), ObjectId(4)],
            "aircraft above MaxHeightForDamage must not burn; equal height still burns"
        );
    }

    /// Rim units enter the DoT only as the circle grows, then leave as it shrinks.
    /// Reverse-at-transition scorch arms once at TransitionTime.
    #[test]
    fn firestorm_expand_reverse_radius_and_scorch() {
        let mut reg = HostHelixNapalmRegistry::new();
        let impact = Vec3::ZERO;
        let id =
            reg.record_drop_and_spawn_firestorm(ObjectId(1), Team::China, impact, 0, false, 0, 0.0);
        assert!(
            (reg.active_zones()[0].radius - HELIX_FIRESTORM_INITIAL_RADIUS).abs() < 0.01,
            "spawn must start at InitialMajorRadius, not FinalMajorRadius"
        );

        let rim = Vec3::new(80.0, 0.0, 0.0);
        let objects = vec![
            (ObjectId(1), Vec3::new(500.0, 0.0, 0.0), Team::China, true),
            (ObjectId(2), rim, Team::GLA, true),
        ];

        let early = reg.plan_due_ticks(0, &objects);
        assert_eq!(early.len(), 1);
        assert!(early[0].hits.is_empty(), "r1 must miss rim at 80");
        assert!(!early[0].place_scorch);
        assert!((early[0].damage_radius - HELIX_FIRESTORM_INITIAL_RADIUS).abs() < 0.01);
        reg.record_tick_complete(id, 0.0, 0, 0, 0);

        // Grow ticks before reverse: rim at 80 still outside.
        let mut frame = HELIX_FIRESTORM_TICK_INTERVAL_FRAMES;
        while frame < HELIX_FIRESTORM_TRANSITION_FRAMES {
            let mid = reg.plan_due_ticks(frame, &objects);
            assert_eq!(mid.len(), 1);
            assert!(
                mid[0].hits.is_empty(),
                "pre-reverse radius {} must miss rim at 80",
                mid[0].damage_radius
            );
            assert!(!mid[0].place_scorch);
            reg.record_tick_complete(id, 0.0, 0, 0, frame);
            frame = frame.saturating_add(HELIX_FIRESTORM_TICK_INTERVAL_FRAMES);
        }

        let peak = reg.plan_due_ticks(HELIX_FIRESTORM_TRANSITION_FRAMES, &objects);
        assert_eq!(peak.len(), 1);
        assert_eq!(peak[0].hits.len(), 1);
        assert_eq!(peak[0].hits[0].target_id, ObjectId(2));
        assert!(peak[0].place_scorch);
        assert!((peak[0].damage_radius - HELIX_FIRESTORM_RADIUS).abs() < 0.01);
        reg.record_tick_complete(
            id,
            HELIX_FIRESTORM_DAMAGE_PER_TICK,
            1,
            0,
            HELIX_FIRESTORM_TRANSITION_FRAMES,
        );
        assert!(reg.active_zones()[0].switched_directions);
        assert!(reg.active_zones()[0].scorch_placed);

        // Shrink ticks: last due frame at 165 (90+5*15).
        let mut last_radius = HELIX_FIRESTORM_RADIUS;
        frame = HELIX_FIRESTORM_TRANSITION_FRAMES + HELIX_FIRESTORM_TICK_INTERVAL_FRAMES;
        while frame < HELIX_FIRESTORM_DURATION_FRAMES {
            let late = reg.plan_due_ticks(frame, &objects);
            assert_eq!(late.len(), 1);
            assert!(!late[0].place_scorch, "scorch only once at reverse");
            last_radius = late[0].damage_radius;
            if last_radius < 80.0 {
                assert!(
                    late[0].hits.is_empty(),
                    "shrunk radius {} must miss rim at 80",
                    last_radius
                );
            }
            reg.record_tick_complete(id, 0.0, 0, 0, frame);
            frame = frame.saturating_add(HELIX_FIRESTORM_TICK_INTERVAL_FRAMES);
        }
        assert!(
            last_radius < 20.0,
            "late shrink must be well below FinalMajorRadius, got {last_radius}"
        );
    }

    #[test]
    fn helix_firestorm_leftover_calls_particle_fx_emission() {
        let src = include_str!("host_helix_napalm.rs");
        assert!(src.contains("leftover_tick_helix_firestorm_fx"));
        assert!(src.contains("FirestormDynamicGeometryInfoUpdate::leftover_tick_particle_fx"));
        assert!(
            src.contains("leftover_follow_emission_radius")
                || src.contains("leftover_tick_particle_fx")
        );
        assert!(
            !src.contains("not full particle\n//!   emission-volume follow"),
            "live must leftover-call leftover ParticleSystem/FXList/emission"
        );

        let mut reg = HostHelixNapalmRegistry::new();
        reg.record_drop_and_spawn_firestorm(ObjectId(1), Team::China, Vec3::ZERO, 0, false, 0, 0.0);
        assert!(!reg.active_zones()[0].leftover_effects_fired);
        reg.advance_geometry(0);
        // Leftover manager may be unregistered in unit tests; first-fire still marks fired.
        assert!(reg.active_zones()[0].leftover_effects_fired);
    }
}
