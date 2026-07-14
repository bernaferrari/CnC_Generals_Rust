//! Host GLA Bomb Truck FireWeaponWhenDead HE/Bio detonation residual.
//!
//! Residual slice (playability):
//! - On death (or residual DetonateNow path), `GLAVehicleBombTruck` (and general
//!   variants) applies exclusive damage + optional poison field residual:
//!   - Default: `BombTruckDefaultBombDamage` Primary **1000**/radius **40** +
//!     Secondary **100**/radius **65**.
//!   - High Explosive upgrade: `BombTruckHighExplosionBombDamage`
//!     Primary **2000**/radius **50** + Secondary **200**/radius **85**.
//!   - BioBomb upgrade: spawn residual MediumPoisonField
//!     (2 dmg / radius 80 / 30s / 500ms ticks).
//!   - Bio + Anthrax Beta: upgraded MediumPoisonField (2.5 dmg / same radius).
//! - HE and Bio may combine (HE blast + bio poison residual).
//! - Honesty counters/flags for residual gates and tests.
//!
//! Fail-closed honesty:
//! - Not full FireWeaponWhenDeadBehavior exclusive module matrix / RequiresAllTriggers
//! - Not full SubObjectsUpgrade Bombload02-04 visual payload swap
//! - Not full Anthrax Gamma / Demo_ red FX particle matrix
//! - Not full WeaponBonus PLAYER_UPGRADE DAMAGE 125% on HE (fail-closed base HE numbers)
//! - Not network bomb-truck detonation replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second.
pub const BOMB_TRUCK_LOGIC_FPS: f32 = 30.0;

/// Retail Upgrade_GLABombTruckHighExplosiveBomb.
pub const UPGRADE_BOMB_TRUCK_HE: &str = "Upgrade_GLABombTruckHighExplosiveBomb";
/// Retail Upgrade_GLABombTruckBioBomb.
pub const UPGRADE_BOMB_TRUCK_BIO: &str = "Upgrade_GLABombTruckBioBomb";
/// Retail Upgrade_GLAAnthraxBeta (upgrades bio poison field).
pub const UPGRADE_GLA_ANTHRAX_BETA: &str = "Upgrade_GLAAnthraxBeta";

/// Default bomb residual (BombTruckDefaultBombDamage).
pub const BOMB_TRUCK_DEFAULT_PRIMARY_DAMAGE: f32 = 1000.0;
pub const BOMB_TRUCK_DEFAULT_PRIMARY_RADIUS: f32 = 40.0;
pub const BOMB_TRUCK_DEFAULT_SECONDARY_DAMAGE: f32 = 100.0;
pub const BOMB_TRUCK_DEFAULT_SECONDARY_RADIUS: f32 = 65.0;

/// High-explosive residual (BombTruckHighExplosionBombDamage).
pub const BOMB_TRUCK_HE_PRIMARY_DAMAGE: f32 = 2000.0;
pub const BOMB_TRUCK_HE_PRIMARY_RADIUS: f32 = 50.0;
pub const BOMB_TRUCK_HE_SECONDARY_DAMAGE: f32 = 200.0;
pub const BOMB_TRUCK_HE_SECONDARY_RADIUS: f32 = 85.0;

/// MediumPoisonField residual (BioBomb OCL_PoisonFieldMedium).
pub const BOMB_TRUCK_POISON_DAMAGE: f32 = 2.0;
pub const BOMB_TRUCK_POISON_DAMAGE_UPGRADED: f32 = 2.5;
pub const BOMB_TRUCK_POISON_RADIUS: f32 = 80.0;
/// DelayBetweenShots 500ms → 15 frames.
pub const BOMB_TRUCK_POISON_TICK_FRAMES: u32 = 15;
/// Lifetime 30000ms → 900 frames.
pub const BOMB_TRUCK_POISON_DURATION_FRAMES: u32 = 900;

/// Detonation audio residual.
pub const BOMB_TRUCK_DEFAULT_DETONATE_AUDIO: &str = "BombTruckDefaultBombDetonation";
pub const BOMB_TRUCK_HE_DETONATE_AUDIO: &str = "BombTruckHighExplosiveBomb";
pub const BOMB_TRUCK_BIO_DETONATE_AUDIO: &str = "BombTruckBioBomb";
pub const BOMB_TRUCK_POISON_AUDIO: &str = "ToxicPoolAmbientLoop";

/// Resolved residual detonation profile from upgrade tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BombTruckDetonationProfile {
    #[default]
    Default,
    HighExplosive,
    Bio,
    Anthrax,
    HighExplosiveBio,
    HighExplosiveAnthrax,
}

impl BombTruckDetonationProfile {
    pub fn from_upgrades(he: bool, bio: bool, anthrax: bool) -> Self {
        match (he, bio, anthrax) {
            (false, false, _) => Self::Default,
            (true, false, _) => Self::HighExplosive,
            (false, true, false) => Self::Bio,
            (false, true, true) => Self::Anthrax,
            (true, true, false) => Self::HighExplosiveBio,
            (true, true, true) => Self::HighExplosiveAnthrax,
        }
    }

    pub fn is_high_explosive(self) -> bool {
        matches!(
            self,
            Self::HighExplosive | Self::HighExplosiveBio | Self::HighExplosiveAnthrax
        )
    }

    pub fn spawns_poison(self) -> bool {
        matches!(
            self,
            Self::Bio | Self::Anthrax | Self::HighExplosiveBio | Self::HighExplosiveAnthrax
        )
    }

    pub fn poison_upgraded(self) -> bool {
        matches!(self, Self::Anthrax | Self::HighExplosiveAnthrax)
    }

    pub fn primary_damage(self) -> f32 {
        if self.is_high_explosive() {
            BOMB_TRUCK_HE_PRIMARY_DAMAGE
        } else {
            BOMB_TRUCK_DEFAULT_PRIMARY_DAMAGE
        }
    }

    pub fn primary_radius(self) -> f32 {
        if self.is_high_explosive() {
            BOMB_TRUCK_HE_PRIMARY_RADIUS
        } else {
            BOMB_TRUCK_DEFAULT_PRIMARY_RADIUS
        }
    }

    pub fn secondary_damage(self) -> f32 {
        if self.is_high_explosive() {
            BOMB_TRUCK_HE_SECONDARY_DAMAGE
        } else {
            BOMB_TRUCK_DEFAULT_SECONDARY_DAMAGE
        }
    }

    pub fn secondary_radius(self) -> f32 {
        if self.is_high_explosive() {
            BOMB_TRUCK_HE_SECONDARY_RADIUS
        } else {
            BOMB_TRUCK_DEFAULT_SECONDARY_RADIUS
        }
    }

    pub fn detonate_audio(self) -> &'static str {
        if self.spawns_poison() {
            BOMB_TRUCK_BIO_DETONATE_AUDIO
        } else if self.is_high_explosive() {
            BOMB_TRUCK_HE_DETONATE_AUDIO
        } else {
            BOMB_TRUCK_DEFAULT_DETONATE_AUDIO
        }
    }
}

/// Whether template is a residual Bomb Truck vehicle.
///
/// Fail-closed: name residual. Reuses disguise residual naming.
pub fn is_bomb_truck_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n == "testbombtruck" || n == "test_bomb_truck" {
        return true;
    }
    // Exclude weapons / effects / death hulks / projectiles (not vehicles).
    if n.contains("damage")
        || n.contains("deatheffect")
        || n.contains("hulk")
        || n.contains("effect")
        || n.contains("weapon")
        || n.contains("projectile")
        || n.contains("shell")
        || n.contains("disguise")
        || n.starts_with("upgrade")
    {
        return false;
    }
    n.contains("vehiclebombtruck")
        || n.contains("vehicle_bombtruck")
        || n.contains("bombtruck")
        || n.contains("bomb_truck")
}

/// Explosive blast damage at distance for a profile (max of primary/secondary rings).
pub fn bomb_truck_blast_damage_at(profile: BombTruckDetonationProfile, distance: f32) -> f32 {
    let p_r = profile.primary_radius();
    let s_r = profile.secondary_radius();
    if distance <= p_r {
        profile.primary_damage()
    } else if distance <= s_r {
        profile.secondary_damage()
    } else {
        0.0
    }
}

/// One active residual MediumPoisonField from BioBomb detonation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostBombTruckPoisonZone {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub radius: f32,
    pub damage_per_tick: f32,
    pub activate_frame: u32,
    pub expires_frame: u32,
    pub next_tick_frame: u32,
    pub upgraded: bool,
    pub total_damage_applied: f32,
    pub damage_applications: u32,
    pub objects_destroyed: u32,
}

impl HostBombTruckPoisonZone {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HostBombTruckPoisonHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub zone_id: u32,
}

#[derive(Debug, Clone)]
pub struct HostBombTruckPoisonTickPlan {
    pub zone_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub hits: Vec<HostBombTruckPoisonHit>,
}

/// Host residual registry for bomb truck detonations + bio poison fields.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBombTruckDetonateRegistry {
    next_id: u32,
    active_poison: Vec<HostBombTruckPoisonZone>,
    /// Total detonations resolved.
    pub detonations: u32,
    /// HE-profile detonations.
    pub he_detonations: u32,
    /// Bio / Anthrax detonations (spawned poison).
    pub bio_detonations: u32,
    /// Total blast damage dealt.
    pub blast_damage_dealt: f32,
    /// Objects hit by blast residual.
    pub blast_hits: u32,
    /// Poison zones spawned.
    pub poison_zones_spawned: u32,
    pub poison_expirations: u32,
    pub poison_damage_applied: f32,
    pub poison_damage_applications: u32,
    pub objects_destroyed: u32,
}

impl HostBombTruckDetonateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_poison_count(&self) -> usize {
        self.active_poison.len()
    }

    pub fn active_poison_zones(&self) -> &[HostBombTruckPoisonZone] {
        &self.active_poison
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Record a detonation blast residual (poison spawn is separate).
    pub fn record_detonation(
        &mut self,
        profile: BombTruckDetonationProfile,
        blast_hits: u32,
        blast_damage: f32,
    ) {
        self.detonations = self.detonations.saturating_add(1);
        self.blast_hits = self.blast_hits.saturating_add(blast_hits);
        if blast_damage > 0.0 {
            self.blast_damage_dealt += blast_damage;
        }
        if profile.is_high_explosive() {
            self.he_detonations = self.he_detonations.saturating_add(1);
        }
        if profile.spawns_poison() {
            self.bio_detonations = self.bio_detonations.saturating_add(1);
        }
    }

    /// Spawn residual MediumPoisonField at detonation site.
    pub fn spawn_poison_field(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        position: Vec3,
        activate_frame: u32,
        upgraded: bool,
    ) -> u32 {
        let id = self.alloc_id();
        let damage = if upgraded {
            BOMB_TRUCK_POISON_DAMAGE_UPGRADED
        } else {
            BOMB_TRUCK_POISON_DAMAGE
        };
        self.active_poison.push(HostBombTruckPoisonZone {
            id,
            source_object,
            source_team,
            position,
            radius: BOMB_TRUCK_POISON_RADIUS,
            damage_per_tick: damage,
            activate_frame,
            expires_frame: activate_frame.saturating_add(BOMB_TRUCK_POISON_DURATION_FRAMES),
            next_tick_frame: activate_frame,
            upgraded,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
        });
        self.poison_zones_spawned = self.poison_zones_spawned.saturating_add(1);
        id
    }

    pub fn plan_due_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostBombTruckPoisonTickPlan> {
        let mut plans = Vec::new();
        for zone in &self.active_poison {
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
                    hits.push(HostBombTruckPoisonHit {
                        target_id: id,
                        damage: zone.damage_per_tick,
                        zone_id: zone.id,
                    });
                }
            }
            plans.push(HostBombTruckPoisonTickPlan {
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
        if let Some(zone) = self.active_poison.iter_mut().find(|z| z.id == zone_id) {
            zone.total_damage_applied += damage_applied;
            zone.damage_applications = zone.damage_applications.saturating_add(applications);
            zone.objects_destroyed = zone.objects_destroyed.saturating_add(destroyed);
            zone.next_tick_frame = current_frame.saturating_add(BOMB_TRUCK_POISON_TICK_FRAMES);
        }
        self.poison_damage_applied += damage_applied;
        self.poison_damage_applications =
            self.poison_damage_applications.saturating_add(applications);
        self.objects_destroyed = self.objects_destroyed.saturating_add(destroyed);
    }

    pub fn prune_expired(&mut self, current_frame: u32) {
        let before = self.active_poison.len();
        self.active_poison.retain(|z| !z.is_expired(current_frame));
        let removed = before.saturating_sub(self.active_poison.len()) as u32;
        self.poison_expirations = self.poison_expirations.saturating_add(removed);
    }

    pub fn honesty_detonate_ok(&self) -> bool {
        self.detonations > 0 && self.blast_damage_dealt > 0.0
    }

    pub fn honesty_he_ok(&self) -> bool {
        self.he_detonations > 0
    }

    pub fn honesty_bio_ok(&self) -> bool {
        self.bio_detonations > 0 && self.poison_zones_spawned > 0
    }

    pub fn honesty_bio_damage_ok(&self) -> bool {
        self.poison_damage_applications > 0 && self.poison_damage_applied > 0.0
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_detonate_ok()
    }
}


// --- Wave 69 residual honesty peels (retail death weapons / poison / upgrades) ---

/// Retail default death weapon name residual.
pub const BOMB_TRUCK_DEFAULT_DAMAGE_WEAPON: &str = "BombTruckDefaultBombDamage";
/// Retail HE death weapon name residual.
pub const BOMB_TRUCK_HE_DAMAGE_WEAPON: &str = "BombTruckHighExplosionBombDamage";
/// Retail DamageType residual.
pub const BOMB_TRUCK_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail DeathType residual.
pub const BOMB_TRUCK_DEATH_TYPE: &str = "EXPLODED";
/// Retail poison field lifetime residual (msec).
pub const BOMB_TRUCK_POISON_DURATION_MS: u32 = 30_000;
/// Retail poison DelayBetweenShots residual (msec).
pub const BOMB_TRUCK_POISON_TICK_MS: u32 = 500;
/// Retail HE object-upgrade BuildCost residual.
pub const BOMB_TRUCK_HE_UPGRADE_COST: u32 = 500;
/// Retail HE object-upgrade BuildTime residual (seconds).
pub const BOMB_TRUCK_HE_UPGRADE_TIME_SEC: f32 = 5.0;
/// HE upgrade → frames.
pub const BOMB_TRUCK_HE_UPGRADE_TIME_FRAMES: u32 = 150;
/// Retail Bio object-upgrade BuildCost residual.
pub const BOMB_TRUCK_BIO_UPGRADE_COST: u32 = 500;
/// Retail Bio object-upgrade BuildTime residual (seconds).
pub const BOMB_TRUCK_BIO_UPGRADE_TIME_SEC: f32 = 5.0;
/// Bio upgrade → frames.
pub const BOMB_TRUCK_BIO_UPGRADE_TIME_FRAMES: u32 = 150;
/// Retail body residual (shared with disguise host).
pub const BOMB_TRUCK_MAX_HEALTH: f32 = 220.0;
pub const BOMB_TRUCK_BUILD_COST: u32 = 1_200;
pub const BOMB_TRUCK_BUILD_TIME_SEC: f32 = 15.0;
pub const BOMB_TRUCK_BUILD_TIME_FRAMES: u32 = 450;
pub const BOMB_TRUCK_VISION_RANGE: f32 = 150.0;
pub const BOMB_TRUCK_SHROUD_CLEARING_RANGE: f32 = 200.0;
pub const BOMB_TRUCK_TRANSPORT_SLOT_COUNT: u32 = 3;

/// Convert residual msec → logic frames @ 30 FPS.
pub fn bomb_truck_detonate_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * BOMB_TRUCK_LOGIC_FPS / 1000.0).round() as u32
}

/// Wave 69 residual honesty: default/HE death weapon residual peel.
pub fn honesty_bomb_truck_detonate_weapon_residual_ok() -> bool {
    BOMB_TRUCK_DEFAULT_DAMAGE_WEAPON == "BombTruckDefaultBombDamage"
        && BOMB_TRUCK_HE_DAMAGE_WEAPON == "BombTruckHighExplosionBombDamage"
        && (BOMB_TRUCK_DEFAULT_PRIMARY_DAMAGE - 1000.0).abs() < 0.01
        && (BOMB_TRUCK_DEFAULT_PRIMARY_RADIUS - 40.0).abs() < 0.01
        && (BOMB_TRUCK_DEFAULT_SECONDARY_DAMAGE - 100.0).abs() < 0.01
        && (BOMB_TRUCK_DEFAULT_SECONDARY_RADIUS - 65.0).abs() < 0.01
        && (BOMB_TRUCK_HE_PRIMARY_DAMAGE - 2000.0).abs() < 0.01
        && (BOMB_TRUCK_HE_PRIMARY_RADIUS - 50.0).abs() < 0.01
        && (BOMB_TRUCK_HE_SECONDARY_DAMAGE - 200.0).abs() < 0.01
        && (BOMB_TRUCK_HE_SECONDARY_RADIUS - 85.0).abs() < 0.01
        && BOMB_TRUCK_DAMAGE_TYPE == "EXPLOSION"
        && BOMB_TRUCK_DEATH_TYPE == "EXPLODED"
        && (bomb_truck_blast_damage_at(BombTruckDetonationProfile::Default, 0.0) - 1000.0).abs()
            < 0.01
        && (bomb_truck_blast_damage_at(BombTruckDetonationProfile::HighExplosive, 0.0) - 2000.0)
            .abs()
            < 0.01
        && bomb_truck_blast_damage_at(BombTruckDetonationProfile::Default, 70.0) <= 0.0
}

/// Wave 69 residual honesty: Bio poison field residual peel.
pub fn honesty_bomb_truck_detonate_poison_residual_ok() -> bool {
    (BOMB_TRUCK_POISON_DAMAGE - 2.0).abs() < 0.01
        && (BOMB_TRUCK_POISON_DAMAGE_UPGRADED - 2.5).abs() < 0.01
        && (BOMB_TRUCK_POISON_RADIUS - 80.0).abs() < 0.01
        && BOMB_TRUCK_POISON_TICK_MS == 500
        && BOMB_TRUCK_POISON_TICK_FRAMES
            == bomb_truck_detonate_ms_to_frames(BOMB_TRUCK_POISON_TICK_MS)
        && BOMB_TRUCK_POISON_TICK_FRAMES == 15
        && BOMB_TRUCK_POISON_DURATION_MS == 30_000
        && BOMB_TRUCK_POISON_DURATION_FRAMES
            == bomb_truck_detonate_ms_to_frames(BOMB_TRUCK_POISON_DURATION_MS)
        && BOMB_TRUCK_POISON_DURATION_FRAMES == 900
        && BOMB_TRUCK_POISON_AUDIO == "ToxicPoolAmbientLoop"
        && BombTruckDetonationProfile::Bio.spawns_poison()
        && BombTruckDetonationProfile::Anthrax.poison_upgraded()
}

/// Wave 69 residual honesty: HE/Bio upgrade residual peel.
pub fn honesty_bomb_truck_detonate_upgrade_residual_ok() -> bool {
    UPGRADE_BOMB_TRUCK_HE == "Upgrade_GLABombTruckHighExplosiveBomb"
        && UPGRADE_BOMB_TRUCK_BIO == "Upgrade_GLABombTruckBioBomb"
        && UPGRADE_GLA_ANTHRAX_BETA == "Upgrade_GLAAnthraxBeta"
        && BOMB_TRUCK_HE_UPGRADE_COST == 500
        && (BOMB_TRUCK_HE_UPGRADE_TIME_SEC - 5.0).abs() < 0.01
        && BOMB_TRUCK_HE_UPGRADE_TIME_FRAMES
            == ((BOMB_TRUCK_HE_UPGRADE_TIME_SEC * BOMB_TRUCK_LOGIC_FPS).round() as u32)
        && BOMB_TRUCK_HE_UPGRADE_TIME_FRAMES == 150
        && BOMB_TRUCK_BIO_UPGRADE_COST == 500
        && (BOMB_TRUCK_BIO_UPGRADE_TIME_SEC - 5.0).abs() < 0.01
        && BOMB_TRUCK_BIO_UPGRADE_TIME_FRAMES == 150
        && BOMB_TRUCK_DEFAULT_DETONATE_AUDIO == "BombTruckDefaultBombDetonation"
        && BOMB_TRUCK_HE_DETONATE_AUDIO == "BombTruckHighExplosiveBomb"
        && BOMB_TRUCK_BIO_DETONATE_AUDIO == "BombTruckBioBomb"
}

/// Wave 69 residual honesty: bomb truck body residual peel.
pub fn honesty_bomb_truck_detonate_body_residual_ok() -> bool {
    (BOMB_TRUCK_MAX_HEALTH - 220.0).abs() < 0.01
        && BOMB_TRUCK_BUILD_COST == 1_200
        && (BOMB_TRUCK_BUILD_TIME_SEC - 15.0).abs() < 0.01
        && BOMB_TRUCK_BUILD_TIME_FRAMES
            == ((BOMB_TRUCK_BUILD_TIME_SEC * BOMB_TRUCK_LOGIC_FPS).round() as u32)
        && BOMB_TRUCK_BUILD_TIME_FRAMES == 450
        && (BOMB_TRUCK_VISION_RANGE - 150.0).abs() < 0.01
        && (BOMB_TRUCK_SHROUD_CLEARING_RANGE - 200.0).abs() < 0.01
        && BOMB_TRUCK_TRANSPORT_SLOT_COUNT == 3
        && is_bomb_truck_template("GLAVehicleBombTruck")
        && !is_bomb_truck_template("BombTruckDefaultBombDamage")
}

/// Combined Wave 69 Bomb Truck detonate residual honesty pack.
pub fn honesty_bomb_truck_detonate_residual_pack_ok() -> bool {
    honesty_bomb_truck_detonate_weapon_residual_ok()
        && honesty_bomb_truck_detonate_poison_residual_ok()
        && honesty_bomb_truck_detonate_upgrade_residual_ok()
        && honesty_bomb_truck_detonate_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn bomb_truck_name_matrix() {
        assert!(is_bomb_truck_template("GLAVehicleBombTruck"));
        assert!(is_bomb_truck_template("Demo_GLAVehicleBombTruck"));
        assert!(is_bomb_truck_template("Chem_GLAVehicleBombTruck"));
        assert!(is_bomb_truck_template("TestBombTruck"));
        assert!(!is_bomb_truck_template("ChinaTankBattleMaster"));
        assert!(!is_bomb_truck_template("BombTruckDefaultBombDamage"));
    }

    #[test]
    fn profile_matrix_from_upgrades() {
        assert_eq!(
            BombTruckDetonationProfile::from_upgrades(false, false, false),
            BombTruckDetonationProfile::Default
        );
        assert_eq!(
            BombTruckDetonationProfile::from_upgrades(true, false, false),
            BombTruckDetonationProfile::HighExplosive
        );
        assert_eq!(
            BombTruckDetonationProfile::from_upgrades(false, true, false),
            BombTruckDetonationProfile::Bio
        );
        assert_eq!(
            BombTruckDetonationProfile::from_upgrades(false, true, true),
            BombTruckDetonationProfile::Anthrax
        );
        assert_eq!(
            BombTruckDetonationProfile::from_upgrades(true, true, false),
            BombTruckDetonationProfile::HighExplosiveBio
        );
        assert_eq!(
            BombTruckDetonationProfile::from_upgrades(true, true, true),
            BombTruckDetonationProfile::HighExplosiveAnthrax
        );
    }

    #[test]
    fn he_blast_larger_than_default() {
        let d = BombTruckDetonationProfile::Default;
        let he = BombTruckDetonationProfile::HighExplosive;
        assert!(he.primary_damage() > d.primary_damage());
        assert!(he.primary_radius() > d.primary_radius());
        assert!((bomb_truck_blast_damage_at(d, 0.0) - 1000.0).abs() < 0.01);
        assert!((bomb_truck_blast_damage_at(he, 0.0) - 2000.0).abs() < 0.01);
        assert!((bomb_truck_blast_damage_at(d, 50.0) - 100.0).abs() < 0.01);
        assert!(bomb_truck_blast_damage_at(d, 70.0) <= 0.0);
    }

    #[test]
    fn detonation_and_poison_honesty() {
        let mut reg = HostBombTruckDetonateRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        reg.record_detonation(BombTruckDetonationProfile::Bio, 2, 1500.0);
        assert!(reg.honesty_detonate_ok());
        assert!(reg.honesty_bio_ok() == false); // zone not spawned yet
        let id = reg.spawn_poison_field(ObjectId(1), Team::GLA, Vec3::ZERO, 0, false);
        assert!(reg.honesty_bio_ok());
        let objects = vec![
            (ObjectId(1), Vec3::new(500.0, 0.0, 0.0), Team::GLA, true),
            (ObjectId(2), Vec3::ZERO, Team::China, true),
        ];
        let plans = reg.plan_due_ticks(0, &objects);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].hits.len(), 1);
        reg.record_tick_complete(id, 2.0, 1, 0, 0);
        assert!(reg.honesty_bio_damage_ok());
        assert!(reg.honesty_host_path_ok());
    }

    #[test]
    fn bomb_truck_detonate_residual_pack_honesty_wave69() {
        assert_eq!(bomb_truck_detonate_ms_to_frames(500), 15);
        assert_eq!(bomb_truck_detonate_ms_to_frames(30_000), 900);
        assert!(honesty_bomb_truck_detonate_weapon_residual_ok());
        assert!(honesty_bomb_truck_detonate_poison_residual_ok());
        assert!(honesty_bomb_truck_detonate_upgrade_residual_ok());
        assert!(honesty_bomb_truck_detonate_body_residual_ok());
        assert!(honesty_bomb_truck_detonate_residual_pack_ok());
        assert_eq!(BOMB_TRUCK_BUILD_TIME_FRAMES, 450);
        assert_eq!(BOMB_TRUCK_DAMAGE_TYPE, "EXPLOSION");
    }
}
