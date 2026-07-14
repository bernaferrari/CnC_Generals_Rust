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
//! - Not full SpecialObject NapalmBomb projectile / HeightDieUpdate fall path
//! - Not full FirestormDynamicGeometryInfoUpdate expand/reverse radius animation
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

/// Retail FirestormSmall FinalMajorRadius residual (fail-closed vs expand anim).
pub const HELIX_FIRESTORM_RADIUS: f32 = 90.0;

/// Retail FirestormSmall DamageAmount per damage frame.
pub const HELIX_FIRESTORM_DAMAGE_PER_TICK: f32 = 100.0;

/// Retail BlackNapalmFirestormSmall DamageAmount.
pub const HELIX_FIRESTORM_DAMAGE_UPGRADED: f32 = 150.0;

/// Retail DelayBetweenDamageFrames = 500 ms → 15 frames @ 30 FPS.
pub const HELIX_FIRESTORM_TICK_INTERVAL_FRAMES: u32 = 15;

/// Retail FirestormSmall LifetimeUpdate 6000 ms → 180 frames @ 30 FPS.
pub const HELIX_FIRESTORM_DURATION_FRAMES: u32 = 180;

/// Retail upgrade that unpauses Helix NapalmBomb special power.
pub const UPGRADE_HELIX_NAPALM_BOMB: &str = "Upgrade_HelixNapalmBomb";

/// Retail BlackNapalm player upgrade (swaps NapalmBomb → BlackNapalmBomb weapon).
pub const UPGRADE_CHINA_BLACK_NAPALM: &str = "Upgrade_ChinaBlackNapalm";

/// Residual weapon names.
pub const NAPALM_BOMB_WEAPON: &str = "NapalmBombWeapon";
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
}

impl HostHelixFirestormZone {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
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
}

/// Host residual registry for Helix NapalmBomb drops + Firestorm zones.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostHelixNapalmRegistry {
    next_id: u32,
    active: Vec<HostHelixFirestormZone>,
    /// Successful napalm drops (special-power activations).
    pub drops: u32,
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

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Record a residual napalm drop and spawn FirestormSmall at impact.
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
            radius: HELIX_FIRESTORM_RADIUS,
            damage_per_tick: damage,
            activate_frame,
            expires_frame: activate_frame.saturating_add(HELIX_FIRESTORM_DURATION_FRAMES),
            // Immediate first tick so residual is host-testable on activation frame.
            next_tick_frame: activate_frame,
            black_napalm,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
        };
        self.active.push(zone);
        self.zones_spawned = self.zones_spawned.saturating_add(1);
        id
    }

    /// Plan Firestorm damage for zones due this frame.
    ///
    /// Retail Firestorm damages ALLIES ENEMIES NEUTRALS; residual skips source Helix.
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
            let r2 = zone.radius * zone.radius;
            for &(id, pos, _team, alive) in object_positions {
                if !alive || id == zone.source_object {
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
            plans.push(HostHelixFirestormTickPlan {
                zone_id: zone.id,
                source_object: zone.source_object,
                source_team: zone.source_team,
                hits,
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

// --- Wave 70 residual honesty packs ---

/// Wave 70 residual honesty: NapalmBomb weapon residual peel.
pub fn honesty_helix_napalm_weapon_residual_ok() -> bool {
    NAPALM_BOMB_WEAPON == "NapalmBombWeapon"
        && BLACK_NAPALM_BOMB_WEAPON == "BlackNapalmBombWeapon"
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
        && HELIX_NAPALM_MAX_SPECIAL_OBJECTS == 1
        && UPGRADE_HELIX_NAPALM_BOMB == "Upgrade_HelixNapalmBomb"
        && UPGRADE_CHINA_BLACK_NAPALM == "Upgrade_ChinaBlackNapalm"
}

/// Wave 70 residual honesty: FirestormSmall DoT residual peel.
pub fn honesty_helix_napalm_firestorm_residual_ok() -> bool {
    (HELIX_FIRESTORM_RADIUS - 90.0).abs() < 0.01
        && (HELIX_FIRESTORM_DAMAGE_PER_TICK - 100.0).abs() < 0.01
        && (HELIX_FIRESTORM_DAMAGE_UPGRADED - 150.0).abs() < 0.01
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
}
