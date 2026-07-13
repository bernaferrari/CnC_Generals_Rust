//! Host GLA Toxin Tractor residual (poison stream + contaminate spray field).
//!
//! Residual slice (playability):
//! - PRIMARY `ToxinTruckGun`: poison stream (dmg **10**, radius **10**, range **100**,
//!   Delay 40ms → 2 frames residual). Anthrax Beta → dmg **12.5**.
//!   Anthrax Gamma (`Chem_ToxinTruckGunGamma`) → dmg **20.5**.
//! - SECONDARY `ToxinTruckSprayer` contaminate residual (special attack only):
//!   SecondaryDamage **2** / radius **75**, range **15**. After residual spray,
//!   spawns MediumPoisonField DoT (2 dmg / radius 80 / 30s / 500ms ticks).
//!   Anthrax Beta/Gamma → spray **2.5** + upgraded MediumPoisonField (**2.5**/tick).
//! - Death residual: `ToxinShellWeapon` → SmallPoisonField (2 dmg / radius 12 /
//!   10s lifetime). Anthrax Beta/Gamma → **2.5**/tick residual.
//! - Chem General toxin trucks start at Anthrax Beta baseline (retail upgraded
//!   WeaponSet) until `Chem_Upgrade_GLAAnthraxGamma` is researched.
//! - Salvage PlusOne/PlusTwo residual: primary damage bump (12.5 / 15).
//!
//! Fail-closed honesty:
//! - Not full FireOCLAfterWeaponCooldown MinShots=4 continuous-coast timer matrix
//! - Not full stream projectile drawing / spigot bone / turret pitch matrix
//! - Not full gamma particle bones / PlusOne-Two anthrax salvage matrix
//! - Not network toxin replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Retail primary stream weapon.
pub const TOXIN_TRUCK_GUN: &str = "ToxinTruckGun";
/// Retail primary after Anthrax Beta.
pub const TOXIN_TRUCK_GUN_UPGRADED: &str = "ToxinTruckGunUpgraded";
/// Retail Chem Anthrax Gamma primary stream.
pub const TOXIN_TRUCK_GUN_GAMMA: &str = "Chem_ToxinTruckGunGamma";
/// Retail secondary contaminate spray.
pub const TOXIN_TRUCK_SPRAYER: &str = "ToxinTruckSprayer";
/// Retail secondary after Anthrax Beta.
pub const TOXIN_TRUCK_SPRAYER_UPGRADED: &str = "ToxinTruckSprayerUpgraded";
/// Retail Chem Anthrax Gamma spray.
pub const TOXIN_TRUCK_SPRAYER_GAMMA: &str = "Chem_ToxinTruckSprayerGamma";
/// Retail Upgrade_GLAAnthraxBeta.
pub const UPGRADE_GLA_ANTHRAX_BETA: &str = "Upgrade_GLAAnthraxBeta";
/// Retail Chem_Upgrade_GLAAnthraxGamma (Chemical General).
pub const UPGRADE_GLA_ANTHRAX_GAMMA: &str = "Chem_Upgrade_GLAAnthraxGamma";
/// Alias residual for shorthand tests / host unlock tags.
pub const UPGRADE_GLA_ANTHRAX_GAMMA_ALT: &str = "Upgrade_GLAAnthraxGamma";

/// Base primary damage / radius / range.
pub const TOXIN_STREAM_DAMAGE: f32 = 10.0;
pub const TOXIN_STREAM_DAMAGE_UPGRADED: f32 = 12.5;
/// Retail Chem_ToxinTruckGunGamma PrimaryDamage.
pub const TOXIN_STREAM_DAMAGE_GAMMA: f32 = 20.5;
pub const TOXIN_STREAM_RADIUS: f32 = 10.0;
pub const TOXIN_STREAM_RANGE: f32 = 100.0;
/// DelayBetweenShots 40ms → 2 frames @ 30 FPS (ceil).
pub const TOXIN_STREAM_DELAY_FRAMES: u32 = 2;

/// Contaminate spray residual (SecondaryDamage / radius / AttackRange).
pub const TOXIN_SPRAY_DAMAGE: f32 = 2.0;
pub const TOXIN_SPRAY_DAMAGE_UPGRADED: f32 = 2.5;
pub const TOXIN_SPRAY_RADIUS: f32 = 75.0;
pub const TOXIN_SPRAY_RANGE: f32 = 15.0;
/// DelayBetweenShots 200ms → 6 frames @ 30 FPS.
pub const TOXIN_SPRAY_DELAY_FRAMES: u32 = 6;

/// MediumPoisonField residual (spray contamination OCL).
pub const TOXIN_MED_FIELD_DAMAGE: f32 = 2.0;
/// Retail Chem_MediumPoisonFieldWeaponGamma / upgraded anthrax residual.
pub const TOXIN_MED_FIELD_DAMAGE_UPGRADED: f32 = 2.5;
pub const TOXIN_MED_FIELD_RADIUS: f32 = 80.0;
/// DelayBetweenShots 500ms → 15 frames.
pub const TOXIN_MED_FIELD_TICK_FRAMES: u32 = 15;
/// Lifetime 30000ms → 900 frames.
pub const TOXIN_MED_FIELD_DURATION_FRAMES: u32 = 900;

/// SmallPoisonField residual (death ToxinShellWeapon OCL).
pub const TOXIN_SMALL_FIELD_DAMAGE: f32 = 2.0;
/// Retail Chem_SmallPoisonFieldWeaponGamma PrimaryDamage residual.
pub const TOXIN_SMALL_FIELD_DAMAGE_UPGRADED: f32 = 2.5;
pub const TOXIN_SMALL_FIELD_RADIUS: f32 = 12.0;
/// Lifetime 10000ms → 300 frames.
pub const TOXIN_SMALL_FIELD_DURATION_FRAMES: u32 = 300;
pub const TOXIN_SMALL_FIELD_TICK_FRAMES: u32 = 15;

/// Salvage PlusOne / PlusTwo primary damage residual (non-anthrax path).
pub const TOXIN_STREAM_DAMAGE_PLUS_ONE: f32 = 12.5;
pub const TOXIN_STREAM_DAMAGE_PLUS_TWO: f32 = 15.0;

/// Residual fire / ambient audio.
pub const TOXIN_STREAM_AUDIO: &str = "ToxinTractorWeaponLoop";
pub const TOXIN_SPRAY_AUDIO: &str = "ToxinTractorContaminate";
pub const TOXIN_POISON_AUDIO: &str = "ToxicPoolAmbientLoop";

/// Anthrax residual combat tier (Beta = stock upgrade; Gamma = Chem general).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AnthraxResidualTier {
    #[default]
    None,
    /// Upgrade_GLAAnthraxBeta / Chem baseline upgraded weapons.
    Beta,
    /// Chem_Upgrade_GLAAnthraxGamma residual.
    Gamma,
}

impl AnthraxResidualTier {
    pub fn is_upgraded(self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn is_gamma(self) -> bool {
        matches!(self, Self::Gamma)
    }
}

/// Salvage residual tier for toxin tractor primary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToxinTractorSalvageTier {
    #[default]
    Base = 0,
    One = 1,
    Two = 2,
}

impl ToxinTractorSalvageTier {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::One,
            2 => Self::Two,
            _ => Self::Base,
        }
    }
}

/// Whether template is a residual Toxin Tractor / Toxin Truck vehicle.
///
/// Fail-closed: name residual (not full Salvage / W3D turret matrix).
/// Excludes weapons, projectiles, poison field system objects.
pub fn is_toxin_tractor_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("stream")
        || n.contains("poisonfield")
        || n.contains("shell")
        || n.starts_with("upgrade")
        || n.contains("sprayer")
        || n.ends_with("gun")
        || n.contains("gungun")
        || n.contains("truckgun")
        || n.contains("trucksprayer")
    {
        return false;
    }
    n.contains("toxintruck")
        || n.contains("toxintrac")
        || n.contains("toxin_truck")
        || n.contains("toxin_tractor")
        || n == "gla_toxintruck"
        || n == "gla_toxintraktor"
        || n == "testtoxintruck"
        || n == "testtoxintraktor"
        || (n.contains("vehicletoxin") && (n.contains("truck") || n.contains("tractor")))
}

/// Whether template is a Chemical General residual unit (Chem_ / GC_Chem_).
///
/// Chem toxin trucks start with upgraded (Anthrax Beta) weapons in retail INI.
pub fn is_chem_general_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.starts_with("chem_") || n.starts_with("gc_chem_") || n.contains("testchem")
}

/// True when upgrade name is Anthrax Gamma residual research.
pub fn is_anthrax_gamma_upgrade_name(name: &str) -> bool {
    let n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    n.contains("anthraxgamma")
}

/// True when upgrade name is Anthrax Beta residual research.
pub fn is_anthrax_beta_upgrade_name(name: &str) -> bool {
    let n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    n.contains("anthraxbeta") && !n.contains("anthraxgamma")
}

/// Resolve anthrax residual tier from upgrade tags + Chem template baseline.
///
/// Fail-closed: not full WeaponSet PLAYER_UPGRADE module matrix / science prereqs.
pub fn anthrax_tier_from_flags(
    has_gamma: bool,
    has_beta: bool,
    chem_template_baseline: bool,
) -> AnthraxResidualTier {
    if has_gamma {
        AnthraxResidualTier::Gamma
    } else if has_beta || chem_template_baseline {
        AnthraxResidualTier::Beta
    } else {
        AnthraxResidualTier::None
    }
}

/// Primary stream damage residual (salvage + anthrax tier).
pub fn toxin_stream_damage(tier: ToxinTractorSalvageTier, anthrax: AnthraxResidualTier) -> f32 {
    match anthrax {
        AnthraxResidualTier::Gamma => TOXIN_STREAM_DAMAGE_GAMMA,
        AnthraxResidualTier::Beta => {
            // Retail upgraded path already includes anthrax damage; salvage Plus residual
            // fail-closed reuses upgraded base (not full PlusOne/Two anthrax matrix).
            TOXIN_STREAM_DAMAGE_UPGRADED
        }
        AnthraxResidualTier::None => match tier {
            ToxinTractorSalvageTier::Base => TOXIN_STREAM_DAMAGE,
            ToxinTractorSalvageTier::One => TOXIN_STREAM_DAMAGE_PLUS_ONE,
            ToxinTractorSalvageTier::Two => TOXIN_STREAM_DAMAGE_PLUS_TWO,
        },
    }
}

/// Contaminate spray secondary damage residual.
pub fn toxin_spray_damage(anthrax: AnthraxResidualTier) -> f32 {
    match anthrax {
        AnthraxResidualTier::None => TOXIN_SPRAY_DAMAGE,
        // Chem gamma spray base is also 2.5 (Plus residual fail-closed).
        AnthraxResidualTier::Beta | AnthraxResidualTier::Gamma => TOXIN_SPRAY_DAMAGE_UPGRADED,
    }
}

/// MediumPoisonField damage-per-tick residual.
pub fn toxin_med_field_damage(anthrax: AnthraxResidualTier) -> f32 {
    match anthrax {
        AnthraxResidualTier::None => TOXIN_MED_FIELD_DAMAGE,
        AnthraxResidualTier::Beta | AnthraxResidualTier::Gamma => TOXIN_MED_FIELD_DAMAGE_UPGRADED,
    }
}

/// SmallPoisonField (death) damage-per-tick residual.
pub fn toxin_small_field_damage(anthrax: AnthraxResidualTier) -> f32 {
    match anthrax {
        AnthraxResidualTier::None => TOXIN_SMALL_FIELD_DAMAGE,
        AnthraxResidualTier::Beta | AnthraxResidualTier::Gamma => TOXIN_SMALL_FIELD_DAMAGE_UPGRADED,
    }
}

/// Whether residual secondary is contaminate spray path (spawn medium field).
pub fn should_apply_toxin_spray(is_toxin_tractor: bool, fired_slot: u8) -> bool {
    is_toxin_tractor && fired_slot == 1
}

/// Whether residual primary stream should apply small splash radius residual.
pub fn should_apply_toxin_stream(is_toxin_tractor: bool, fired_slot: u8) -> bool {
    is_toxin_tractor && fired_slot == 0
}

/// Stream residual damage at distance (primary radius ring).
pub fn toxin_stream_damage_at(distance: f32, base_damage: f32) -> f32 {
    if distance <= TOXIN_STREAM_RADIUS {
        base_damage
    } else {
        0.0
    }
}

/// Spray residual damage at distance from tractor (SecondaryDamageRadius).
pub fn toxin_spray_damage_at(distance: f32, spray_damage: f32) -> f32 {
    if distance <= TOXIN_SPRAY_RADIUS {
        spray_damage
    } else {
        0.0
    }
}

/// Legal residual toxin splash / field target (not airborne residual).
pub fn is_legal_toxin_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
    is_airborne: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind && !is_airborne
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// One active residual poison field (medium spray or small death).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostToxinTractorPoisonZone {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub radius: f32,
    pub damage_per_tick: f32,
    pub activate_frame: u32,
    pub expires_frame: u32,
    pub next_tick_frame: u32,
    /// Anthrax residual tier for this field (Beta/Gamma use upgraded DoT).
    pub anthrax_tier: AnthraxResidualTier,
    /// True when spawned by death residual (small field).
    pub from_death: bool,
    pub total_damage_applied: f32,
    pub damage_applications: u32,
    pub objects_destroyed: u32,
}

impl HostToxinTractorPoisonZone {
    /// Backward-compat residual flag (any anthrax upgrade).
    pub fn anthrax_upgraded(&self) -> bool {
        self.anthrax_tier.is_upgraded()
    }

    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }
}

/// Damage application plan for a single victim this tick.
#[derive(Debug, Clone, Copy)]
pub struct HostToxinTractorPoisonHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub zone_id: u32,
}

/// Result of resolving one poison zone's damage tick.
#[derive(Debug, Clone)]
pub struct HostToxinTractorPoisonTickPlan {
    pub zone_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub hits: Vec<HostToxinTractorPoisonHit>,
}

/// Host residual registry for Toxin Tractor poison fields + honesty counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostToxinTractorRegistry {
    next_id: u32,
    active: Vec<HostToxinTractorPoisonZone>,
    pub zones_spawned: u32,
    pub death_fields_spawned: u32,
    pub expirations: u32,
    pub total_damage_applied: f32,
    pub damage_applications: u32,
    pub objects_destroyed: u32,
    /// Primary stream residual fires.
    pub stream_fires: u32,
    /// Units hit by stream residual (including intended).
    pub stream_units_hit: u32,
    /// Contaminate spray residual fires.
    pub spray_fires: u32,
    /// Units hit by spray residual splash.
    pub spray_units_hit: u32,
    /// Salvage tier apply count.
    pub salvage_upgrades: u32,
    /// Anthrax Gamma residual stream fires (observable honesty).
    pub gamma_stream_fires: u32,
    /// Anthrax Gamma residual field spawns (spray or death).
    pub gamma_fields_spawned: u32,
}

impl HostToxinTractorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn active_zones(&self) -> &[HostToxinTractorPoisonZone] {
        &self.active
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    pub fn record_stream_fire(&mut self, units_hit: u32) {
        self.stream_fires = self.stream_fires.saturating_add(1);
        self.stream_units_hit = self.stream_units_hit.saturating_add(units_hit);
    }

    pub fn record_spray_fire(&mut self, units_hit: u32) {
        self.spray_fires = self.spray_fires.saturating_add(1);
        self.spray_units_hit = self.spray_units_hit.saturating_add(units_hit);
    }

    pub fn record_salvage_upgrade(&mut self) {
        self.salvage_upgrades = self.salvage_upgrades.saturating_add(1);
    }

    /// Spawn residual MediumPoisonField at contaminate spray location.
    pub fn spawn_medium_field(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        impact_pos: Vec3,
        activate_frame: u32,
        anthrax: AnthraxResidualTier,
    ) -> u32 {
        let id = self.alloc_id();
        let zone = HostToxinTractorPoisonZone {
            id,
            source_object,
            source_team,
            position: impact_pos,
            radius: TOXIN_MED_FIELD_RADIUS,
            damage_per_tick: toxin_med_field_damage(anthrax),
            activate_frame,
            expires_frame: activate_frame.saturating_add(TOXIN_MED_FIELD_DURATION_FRAMES),
            next_tick_frame: activate_frame,
            anthrax_tier: anthrax,
            from_death: false,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
        };
        self.active.push(zone);
        self.zones_spawned = self.zones_spawned.saturating_add(1);
        if anthrax.is_gamma() {
            self.gamma_fields_spawned = self.gamma_fields_spawned.saturating_add(1);
        }
        id
    }

    /// Spawn residual SmallPoisonField on toxin tractor death.
    pub fn spawn_death_field(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        death_pos: Vec3,
        activate_frame: u32,
        anthrax: AnthraxResidualTier,
    ) -> u32 {
        let id = self.alloc_id();
        let zone = HostToxinTractorPoisonZone {
            id,
            source_object,
            source_team,
            position: death_pos,
            radius: TOXIN_SMALL_FIELD_RADIUS,
            damage_per_tick: toxin_small_field_damage(anthrax),
            activate_frame,
            expires_frame: activate_frame.saturating_add(TOXIN_SMALL_FIELD_DURATION_FRAMES),
            next_tick_frame: activate_frame,
            anthrax_tier: anthrax,
            from_death: true,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
        };
        self.active.push(zone);
        self.zones_spawned = self.zones_spawned.saturating_add(1);
        self.death_fields_spawned = self.death_fields_spawned.saturating_add(1);
        if anthrax.is_gamma() {
            self.gamma_fields_spawned = self.gamma_fields_spawned.saturating_add(1);
        }
        id
    }

    pub fn record_gamma_stream_fire(&mut self) {
        self.gamma_stream_fires = self.gamma_stream_fires.saturating_add(1);
    }

    pub fn honesty_gamma_ok(&self) -> bool {
        self.gamma_stream_fires > 0 || self.gamma_fields_spawned > 0
    }

    pub fn plan_due_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool, bool)],
    ) -> Vec<HostToxinTractorPoisonTickPlan> {
        // object_positions: (id, pos, team, alive, airborne)
        let mut plans = Vec::new();
        for zone in &self.active {
            if !zone.is_due_tick(current_frame) {
                continue;
            }
            let mut hits = Vec::new();
            for &(id, pos, _team, alive, airborne) in object_positions {
                if !alive || id == zone.source_object || airborne {
                    continue;
                }
                let dx = zone.position.x - pos.x;
                let dz = zone.position.z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist <= zone.radius {
                    hits.push(HostToxinTractorPoisonHit {
                        target_id: id,
                        damage: zone.damage_per_tick,
                        zone_id: zone.id,
                    });
                }
            }
            plans.push(HostToxinTractorPoisonTickPlan {
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
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(zone) = self.active.iter_mut().find(|z| z.id == zone_id) {
            zone.total_damage_applied += total_damage;
            zone.damage_applications += applications;
            zone.objects_destroyed += objects_destroyed;
            let interval = if zone.from_death {
                TOXIN_SMALL_FIELD_TICK_FRAMES
            } else {
                TOXIN_MED_FIELD_TICK_FRAMES
            };
            zone.next_tick_frame = current_frame.saturating_add(interval);
            self.total_damage_applied += total_damage;
            self.damage_applications = self.damage_applications.saturating_add(applications);
            self.objects_destroyed = self.objects_destroyed.saturating_add(objects_destroyed);
        }
    }

    pub fn prune_expired(&mut self, current_frame: u32) {
        let before = self.active.len();
        self.active.retain(|z| !z.is_expired(current_frame));
        self.expirations = self
            .expirations
            .saturating_add((before.saturating_sub(self.active.len())) as u32);
    }

    pub fn honesty_stream_ok(&self) -> bool {
        self.stream_fires > 0
    }

    pub fn honesty_spray_ok(&self) -> bool {
        self.spray_fires > 0 && self.zones_spawned > 0
    }

    pub fn honesty_death_field_ok(&self) -> bool {
        self.death_fields_spawned > 0
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_stream_ok() || self.honesty_spray_ok() || self.honesty_death_field_ok()
    }
}

/// 2D distance residual.
pub fn in_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn toxin_tractor_name_matrix() {
        assert!(is_toxin_tractor_template("GLAVehicleToxinTruck"));
        assert!(is_toxin_tractor_template("GLA_ToxinTruck"));
        assert!(is_toxin_tractor_template("TestToxinTruck"));
        assert!(is_toxin_tractor_template("Chem_GLAVehicleToxinTruck"));
        assert!(is_toxin_tractor_template("Demo_GLAVehicleToxinTruck"));
        assert!(is_toxin_tractor_template("Slth_GLAVehicleToxinTruck"));
        assert!(!is_toxin_tractor_template("ToxinTruckGun"));
        assert!(!is_toxin_tractor_template("ToxinTruckSprayer"));
        assert!(!is_toxin_tractor_template("PoisonFieldMedium"));
        assert!(!is_toxin_tractor_template("ToxinShellWeapon"));
        assert!(!is_toxin_tractor_template("GLAVehicleScudLauncher"));
        assert!(!is_toxin_tractor_template("USA_Ranger"));
    }

    #[test]
    fn stream_and_spray_stats() {
        assert!(
            (toxin_stream_damage(ToxinTractorSalvageTier::Base, AnthraxResidualTier::None) - 10.0)
                .abs()
                < 0.01
        );
        assert!(
            (toxin_stream_damage(ToxinTractorSalvageTier::Base, AnthraxResidualTier::Beta) - 12.5)
                .abs()
                < 0.01
        );
        assert!(
            (toxin_stream_damage(ToxinTractorSalvageTier::Base, AnthraxResidualTier::Gamma)
                - 20.5)
                .abs()
                < 0.01
        );
        assert!(
            (toxin_stream_damage(ToxinTractorSalvageTier::Two, AnthraxResidualTier::None) - 15.0)
                .abs()
                < 0.01
        );
        assert!((toxin_spray_damage(AnthraxResidualTier::None) - 2.0).abs() < 0.01);
        assert!((toxin_spray_damage(AnthraxResidualTier::Beta) - 2.5).abs() < 0.01);
        assert!((toxin_spray_damage(AnthraxResidualTier::Gamma) - 2.5).abs() < 0.01);
        assert!((toxin_med_field_damage(AnthraxResidualTier::None) - 2.0).abs() < 0.01);
        assert!((toxin_med_field_damage(AnthraxResidualTier::Gamma) - 2.5).abs() < 0.01);
        assert!((toxin_stream_damage_at(5.0, 10.0) - 10.0).abs() < 0.01);
        assert!((toxin_stream_damage_at(15.0, 10.0)).abs() < 0.01);
        assert!((toxin_spray_damage_at(50.0, 2.0) - 2.0).abs() < 0.01);
        assert!((toxin_spray_damage_at(80.0, 2.0)).abs() < 0.01);
        assert!(should_apply_toxin_spray(true, 1));
        assert!(!should_apply_toxin_spray(true, 0));
        assert!(should_apply_toxin_stream(true, 0));
        assert!(is_chem_general_template("Chem_GLAVehicleToxinTruck"));
        assert!(is_anthrax_gamma_upgrade_name("Chem_Upgrade_GLAAnthraxGamma"));
        assert!(is_anthrax_gamma_upgrade_name("Upgrade_GLAAnthraxGamma"));
        assert!(!is_anthrax_gamma_upgrade_name("Upgrade_GLAAnthraxBeta"));
        assert_eq!(
            anthrax_tier_from_flags(false, false, true),
            AnthraxResidualTier::Beta
        );
        assert_eq!(
            anthrax_tier_from_flags(true, true, true),
            AnthraxResidualTier::Gamma
        );
    }

    #[test]
    fn registry_spawn_and_honesty() {
        let mut reg = HostToxinTractorRegistry::new();
        let id = reg.spawn_medium_field(
            ObjectId(1),
            Team::GLA,
            Vec3::ZERO,
            0,
            AnthraxResidualTier::Gamma,
        );
        assert_eq!(id, 0);
        assert!((reg.active_zones()[0].damage_per_tick - 2.5).abs() < 0.01);
        assert!(reg.active_zones()[0].anthrax_upgraded());
        assert!(reg.honesty_gamma_ok());
        reg.record_stream_fire(1);
        assert!(reg.honesty_stream_ok());
        let _ = reg.spawn_medium_field(
            ObjectId(1),
            Team::GLA,
            Vec3::ZERO,
            0,
            AnthraxResidualTier::None,
        );
        reg.record_spray_fire(2);
        assert!(reg.honesty_spray_ok());
        let _ = reg.spawn_death_field(
            ObjectId(1),
            Team::GLA,
            Vec3::ZERO,
            0,
            AnthraxResidualTier::None,
        );
        assert!(reg.honesty_death_field_ok());
        assert!(reg.honesty_host_path_ok());
        // gamma medium + base medium + death field
        assert_eq!(reg.active_count(), 3);
    }
}
